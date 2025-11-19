# shell.nix - For use with nix-shell on systems without flake support
#
# Usage:
#   nix-shell              # Enter development shell
#   nix-shell --pure       # Enter pure development shell
#
# This file uses flake-compat to provide the same devShell as the flake.

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
}).shellNix
