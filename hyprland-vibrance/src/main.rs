use std::{borrow::Borrow, sync::Arc};

use clap::Parser;
use derive_new::new;
use log::{LevelFilter, debug, error, info};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    backend::{ObjectData, ObjectId},
    protocol::{
        wl_output::{self, WlOutput},
        wl_registry::{self},
    },
};
use wayland_protocols_hyprland::ctm_control::v1::client::hyprland_ctm_control_manager_v1::{
    self, HyprlandCtmControlManagerV1,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

const HYPERLAND_CTM_CONTROL_MANAGER_IFACE: &str = "hyprland_ctm_control_manager_v1";
const ZWLR_TOP_LEVEL_MANAGER_IFACE: &str = "zwlr_foreign_toplevel_manager_v1";
const WL_OUTPUT_IFACE: &str = "wl_output";

#[derive(new, Debug, Clone)]
struct Global {
    name: u32,
    interface: String,
    version: u32,
}

#[derive(Debug)]
struct TopLevelInfo {
    handle: ZwlrForeignToplevelHandleV1,
    title: Option<String>,
    current_outputs: Vec<WlOutput>,
}

impl TopLevelInfo {
    pub fn new(handle: ZwlrForeignToplevelHandleV1) -> Self {
        return Self {
            handle,
            title: None,
            current_outputs: Vec::new(),
        };
    }
}

impl TopLevelInfo {
    pub fn push_current_output(&mut self, handle: WlOutput) {
        if !self.current_outputs.contains(&handle) {
            self.current_outputs.push(handle);
        }
    }
    pub fn pop_current_output(&mut self, handle: &WlOutput) {
        if let Some(idx) = self.current_outputs.iter().position(|e| e == handle) {
            self.current_outputs.remove(idx);
        }
    }
}

struct TopLevelUserData;

#[derive(Debug, Default)]
struct InitAppState {
    ctm_manager: Option<HyprlandCtmControlManagerV1>,
    top_level_manager_global: Option<Global>,
}

#[derive(Debug, Default)]
struct AppState {
    init: Option<Box<InitAppState>>,
    top_levels: Vec<TopLevelInfo>,
    focused_top_level_object_id: Option<ObjectId>,
}

fn format_top_level(top_level: &TopLevelInfo) -> String {
    format!(
        "<{}>[{}]",
        top_level.handle.id(),
        top_level
            .title
            .as_ref()
            .map(|t| t.as_str())
            .unwrap_or("<no title>")
    )
}

impl AppState {
    fn index_of_top_level_for_object_id(&self, id: &ObjectId) -> Option<usize> {
        self.top_levels.iter().position(|e| &e.handle.id() == id)
    }

    pub fn focused_top_level(&self) -> Option<&TopLevelInfo> {
        self.focused_top_level_object_id
            .as_ref()
            .and_then(|focused_id| self.index_of_top_level_for_object_id(focused_id))
            .map(|focused_idx| &self.top_levels[focused_idx])
    }

    pub fn get_or_create_top_level<'a>(
        &'a mut self,
        handle: &ZwlrForeignToplevelHandleV1,
    ) -> &'a mut TopLevelInfo {
        let existing_idx = self.index_of_top_level_for_object_id(&handle.id());
        let new_insert_idx = self.top_levels.len();
        if existing_idx.is_none() {
            self.top_levels.push(TopLevelInfo::new(handle.clone()));
        }

        &mut self.top_levels[existing_idx.unwrap_or(new_insert_idx)]
    }

    pub fn notify_top_level_focus_changed(
        &mut self,
        changed_handle: &ZwlrForeignToplevelHandleV1,
        focused: bool,
    ) {
        if focused {
            self.focused_top_level_object_id = Some(changed_handle.id());
        } else if self.focused_top_level_object_id == Some(changed_handle.id()) {
            self.focused_top_level_object_id = None;
        }
    }

    pub fn notify_top_level_closed(&mut self, handle: &ZwlrForeignToplevelHandleV1) {
        if let Some(idx) = self.index_of_top_level_for_object_id(&handle.id()) {
            if Some(handle.id()) == self.focused_top_level_object_id {
                self.focused_top_level_object_id.take();
            }
            self.top_levels.remove(idx);
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for AppState {
    fn event(
        this: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<AppState>,
    ) {
        // It seems that we're not being able to fetch the current
        // output for a top level unless we're bound to such
        // output. Since it can happen that we bound first to the top
        // level manager before than to the outputs, it may happen
        // that the top levels get stuck in an unknown output until
        // they got moved. For avoiding that, we're delying the
        // binding to the top level manager until we've received the
        // first set of globals.
        debug!("Received globals event: {:?}", event);

        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };

        if interface == WL_OUTPUT_IFACE {
            registry.bind::<WlOutput, _, _>(name, version, qh, ());
        }

        let Some(init) = this.init.as_mut() else {
            return;
        };

        match &interface[..] {
            HYPERLAND_CTM_CONTROL_MANAGER_IFACE => {
                init.ctm_manager = Some(registry.bind(name, version, qh, ()));
                info!("Bound to Hyprland CTM control manager");
            }
            ZWLR_TOP_LEVEL_MANAGER_IFACE => {
                init.top_level_manager_global = Some(Global::new(name, interface, version));
                info!("Discovered to wlr top level manager");
            }
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, ()> for AppState {
    fn event(
        _: &mut Self,
        output: &WlOutput,
        event: <WlOutput as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            debug!("Discovered display {}: {}", output.id(), name);
        }
    }
}

impl Dispatch<HyprlandCtmControlManagerV1, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &HyprlandCtmControlManagerV1,
        _: hyprland_ctm_control_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
    }
}

impl ObjectData for TopLevelUserData {
    fn event(
        self: Arc<Self>,
        _: &wayland_client::backend::Backend,
        _: wayland_client::backend::protocol::Message<
            wayland_client::backend::ObjectId,
            std::os::unix::prelude::OwnedFd,
        >,
    ) -> Option<Arc<dyn ObjectData>> {
        None
    }

    fn destroyed(&self, _object_id: wayland_client::backend::ObjectId) {}
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for AppState {
    fn event(
        this: &mut Self,
        _output: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
        match event {
            zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } => {
                debug!("New top level found: {}", toplevel.id());
                this.get_or_create_top_level(&toplevel);
            }
            _ => {}
        }
    }

    // There's a macro to implement this function, but leaves the code
    // quite unclear, and doesn't provide a lot of benefit.
    fn event_created_child(opcode: u16, qh: &QueueHandle<Self>) -> std::sync::Arc<dyn ObjectData> {
        if opcode == zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE {
            qh.make_data(TopLevelUserData)
        } else {
            panic!(
                "Unexpected opcode for child creation event in ZwlrForeignToplevelManagerV1: {}",
                opcode
            );
        }
    }
}

impl Dispatch<ZwlrForeignToplevelHandleV1, TopLevelUserData> for AppState {
    fn event(
        this: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &TopLevelUserData,
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
        let top_level = this.get_or_create_top_level(handle);
        match event {
            zwlr_foreign_toplevel_handle_v1::Event::Title { title } => {
                debug!(
                    "Top level {} title updated: '{}'",
                    format_top_level(&top_level),
                    title
                );
                top_level.title = Some(title);
            }
            zwlr_foreign_toplevel_handle_v1::Event::OutputEnter { output } => {
                debug!(
                    "Top level {} moved to new display: {}",
                    format_top_level(&top_level),
                    output.id()
                );
                top_level.push_current_output(output);
            }
            zwlr_foreign_toplevel_handle_v1::Event::OutputLeave { output } => {
                debug!(
                    "Top level {} left display: {}",
                    format_top_level(&top_level),
                    output.id()
                );
                top_level.pop_current_output(&output);
            }
            zwlr_foreign_toplevel_handle_v1::Event::State { state } => {
                debug!(
                    "Top level {} state update: {:?}",
                    format_top_level(&top_level),
                    state
                );
                let focused =
                    state.contains(&(zwlr_foreign_toplevel_handle_v1::State::Activated as u8));
                this.notify_top_level_focus_changed(&handle, focused);
            }
            zwlr_foreign_toplevel_handle_v1::Event::Closed => {
                debug!("Top level {} state closed", format_top_level(&top_level));
                this.notify_top_level_closed(handle);
            }
            _ => {}
        }
    }
}

// between 0.0 and 4.0. Evily stolen from libvibrant
fn calc_ctm_matrix(saturation: f64) -> [f64; 9] {
    let mut matrix = [0f64; 9];
    let coeff = (1.0 - saturation) / 3.0;
    for i in 0..9 {
        matrix[i] = coeff + if (i % 4) == 0 { saturation } else { 0f64 };
    }

    return matrix;
}

fn clear_ctm_matrix_for_display(control: &HyprlandCtmControlManagerV1, display: &WlOutput) {
    control.set_ctm_for_output(display, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
}

fn set_sat_ctm_for_display(
    control: &HyprlandCtmControlManagerV1,
    display: &WlOutput,
    saturation: f64,
) {
    let matrix = calc_ctm_matrix(saturation);
    control.set_ctm_for_output(
        display, matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5], matrix[6],
        matrix[7], matrix[8],
    );
}

fn diff_lists<'a, A: Eq, E1: Borrow<A>, E2: Borrow<A>>(
    old: &'a [E1],
    new: &'a [E2],
) -> (Vec<&'a A>, Vec<&'a A>, Vec<&'a A>) {
    fn contains_ref<A: Eq>(haystack: &[impl Borrow<A>], needle: &A) -> bool {
        for elem in haystack {
            if elem.borrow() == needle {
                return true;
            }
        }

        return false;
    }

    let mut removed = vec![];
    let mut unchanged = vec![];
    let mut added = vec![];

    for old_value in old.iter() {
        if contains_ref(new, old_value.borrow()) {
            unchanged.push(old_value.borrow());
        } else {
            removed.push(old_value.borrow());
        }
    }

    for new_value in new.iter() {
        if !contains_ref(old, new_value.borrow()) {
            added.push(new_value.borrow());
        }
    }

    return (removed, unchanged, added);
}
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Saturation level (must be between 0.0 and 4.0)
    #[arg(short, long, value_parser = validate_sat_level)]
    sat_level: f64,

    /// Title match filters (can be used multiple times)
    #[arg(short, long, num_args = 1.., value_name = "TITLE", required = true)]
    title_match: Vec<String>,
}

fn validate_sat_level(s: &str) -> Result<f64, String> {
    let val: f64 = s
        .parse()
        .map_err(|_| format!("'{}' is not a valid float", s))?;
    if (0.0..=4.0).contains(&val) {
        Ok(val)
    } else {
        Err(format!(
            "sat-level must be between 0.0 and 4.0, got {}",
            val
        ))
    }
}

fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    let args = Cli::parse();

    let conn = Connection::connect_to_env().unwrap();
    let display = conn.display();
    let mut event_queue = conn.new_event_queue();
    let qh: QueueHandle<AppState> = event_queue.handle();

    let mut state = AppState::default();
    state.init = Some(Box::new(InitAppState::default()));

    let registry = display.get_registry(&qh, ());
    event_queue.roundtrip(&mut state).unwrap();

    let init_state = state.init.take().unwrap();
    let Some(ctm_control) = init_state.ctm_manager else {
        error!(
            "Couldn't find Hyprland CTM control manager interface. Are you actually running Hyprland?"
        );
        return;
    };

    let Some(top_level_manager_global) = init_state.top_level_manager_global else {
        error!("Couldn't find wlr top level manager interface");
        return;
    };

    registry.bind::<ZwlrForeignToplevelManagerV1, _, _>(
        top_level_manager_global.name,
        top_level_manager_global.version,
        &qh,
        (),
    );
    info!("Bound to top level manager interface");

    info!("CTM control initialized successfully");
    let mut outputs_with_custom_ctm: Vec<WlOutput> = Vec::new();
    const SATURATION: f64 = 3.3;

    loop {
        event_queue.blocking_dispatch(&mut state).unwrap();
        let desired_outputs_with_custom_ctm: &[WlOutput] = state
            .focused_top_level()
            .filter(|top_level| {
                top_level
                    .title
                    .as_ref()
                    .map_or(false, |title| args.title_match.contains(title))
            })
            .map_or(&[], |top_level: &TopLevelInfo| {
                top_level.current_outputs.as_ref()
            });

        let (removed_outputs, unchanged_outputs, added_outputs) =
            diff_lists(&outputs_with_custom_ctm, desired_outputs_with_custom_ctm);

        for removed_output in removed_outputs.iter() {
            clear_ctm_matrix_for_display(&ctm_control, removed_output);
        }

        for added_output in added_outputs.iter() {
            set_sat_ctm_for_display(&ctm_control, &added_output, SATURATION);
        }

        if !removed_outputs.is_empty() || !added_outputs.is_empty() {
            ctm_control.commit();
            outputs_with_custom_ctm = unchanged_outputs
                .iter()
                .map(|&output| output.to_owned())
                .chain(added_outputs.iter().map(|&output| output.to_owned()))
                .collect();
        }
    }
}
