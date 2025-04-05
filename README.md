# Hyprland (Custom build) CTM vibrance

This a quick, low effort repository that includes two main things:
* A hyprland-vibrance program, developed in Rust, that allows
  adjusting the saturation of a display based on the current focused
  program. It works similarly to Vibrance or vibrantLinux (Indeed,
  there's code "inspired" from the latter here), but specifically
  developed for working in Hyprland under Wayland. It should work on
  any graphics card that takes into consideration the CTM (Color
  transformation matrix) kernel value from the DRM. (probably
  non-Nvidia GPUs).
* A custom patch for Hyprland so the program above can actually work.

## Why?

vibrantLinux works under X11 for some GPUs because X11 exposes the
kernel CTM setting to any program in the userspace, like
`xprop`. However, there's no standard way of doing this on
Wayland. _However_, Hyprland has implemented a [custom
protocol](https://github.com/hyprwm/hyprland-protocols/blob/main/protocols/hyprland-ctm-control-v1.xml)
that actually gives support for the manipulation of this CTM setting
from any Wayland client.

Unfortunately, on the time of writing this, there's a limitation in
the protocol that prevents providing negative values to the matrix
(not sure why), which is required for properly manipulating the
saturation as vibrantLinux does. Therefore, the patch included on this
repository simply gets rid of the negative values check in the
Hyprland codebase.

## Running this thingy

I'm only providing easy support for this to be built in Arch
Linux. Follow these steps:
 - Clone this repo: `git clone --recursive git@github.com:devcexx/hyprland-ctm-vibrance.git`
 - Make sure you are checking out the last Arch linux Hyprland distribution: `(cd hyprland-build/custom-pkgbuild && git pull)`
 - Build and install your custom Hyprland package (make sure you
   update it regularly!): `(cd hyprland-build && ./gen-pkgbuild.sh &&
   cd custom-pkgbuild && makepkg -sif)`
 - Restart Hyprland
 - Build the Rust application: `(cd hyprland-vibrance && cargo build --release)`
 - Run the program: `hyprland-vibrance/target/release/hyprland-vibrance --saturation 3.3 --title-match "Counter-Strike 2"`

By default, Hyprland also performs an animation while switching from a
CTM value to another. On my case, that lags the whole compositor for
the time the animation takes. For avoinding that, add
`render:ctm_animation = 0` to your Hyprland config.
