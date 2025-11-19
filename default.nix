# default.nix - For use with nix-build on systems without flake support
#
# Usage:
#   nix-build              # Build the default package
#   nix-build -A minimal   # Build minimal package (IRC only)
#
# This file uses flake-compat to provide the same packages as the flake.

(import (
  let
    lock = builtins.fromJSON (builtins.readFile ./flake.lock);
  in
  fetchTarball {
    url = "https://github.com/edolstra/flake-compat/archive/${lock.nodes.flake-compat.locked.rev}.tar.gz";
    sha256 = lock.nodes.flake-compat.locked.narHash;
  }
) {
  src = ./.;
}).defaultNix
