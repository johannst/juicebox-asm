# SPDX-License-Identifier: MIT
#
# Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>
{
  description = "rv32i development environment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs = { self, nixpkgs, ... }: let
    system = "x86_64-linux";
    pkgs   = import nixpkgs { inherit system; };
  in {
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = [
        pkgs.zig
        pkgs.zls
      ];
      shellHook = ''
        # https://github.com/NixOS/nixpkgs/issues/270415
        unset ZIG_GLOBAL_CACHE_DIR
      '';
    };
  };
}
