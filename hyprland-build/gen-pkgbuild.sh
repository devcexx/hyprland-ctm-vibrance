#!/bin/bash
print_var() {
    var_name="$1"
    decl=$(declare -p $var_name 2>/dev/null)
    if [ $? -ne 0 ]; then
	# Undefined variable. Skipping
	return
    fi

    if [[ "$decl" =~ "declare -a" ]]; then
	array_ref=$var_name[@]
	echo "$1=(${!array_ref@Q})"
    else
	echo "$1=${!var_name@Q}"
    fi
}

original_folder="original-arch-pkgbuild/"
src_pkgbuild="$original_folder/PKGBUILD"

output_folder="custom-pkgbuild/"
dst_pkgbuild="$output_folderPKGBUILD"
hyprland_patch="remove-ctm-negative-values-check.patch"

rm -rf "$output_folder"
cp -r "$original_folder" "$output_folder"
cp "$hyprland_patch" "$output_folder"
rm "$dst_pkgbuild"

source "$src_pkgbuild"
pkgname="hyprland-custom"
url="https://github.com/hyprwm/Hyprland"
_archive="Hyprland-custom-$pkgver"
provides+=("hyprland")
conflicts+=("hyprland")
source+=("$hyprland_patch")
sha256sums+=("SKIP")

orig_prepare_function_body="$(declare -f prepare | sed '1,2d;$d')"
new_prepare_function_body="$orig_prepare_function_body
    git apply \"\$srcdir/$hyprland_patch\"
"

new_prepare_function_decl="prepare() {
$new_prepare_function_body
}"

(
    print_var pkgname
    print_var pkgver
    print_var pkgrel
    print_var pkgdesc
    print_var url
    print_var arch
    print_var license
    print_var groups
    print_var makedepends
    print_var checkdepends
    print_var depends
    print_var optdepends
    print_var provides
    print_var conflicts
    print_var replaces
    print_var _archive
    print_var source
    print_var noextract
    print_var validpgpkeys
    print_var sha224sums
    print_var sha256sums
    print_var sha384sums
    print_var sha512sums
    print_var sha1sums
    print_var md5sums
    print_var cksums
    print_var b2sums

    echo "$new_prepare_function_decl"
    declare -f build package
) > "$dst_pkgbuild"

echo "Generated PKGBUILD at $dst_pkgbuild" >&2
