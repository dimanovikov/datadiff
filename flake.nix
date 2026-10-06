{
  description = "Semantic diff for JSON, YAML, CSV, TOML and XML that plugs into git diff";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      # Version and metadata come from Cargo.toml so a release only bumps it there.
      cargoPackage = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package;
    in
    {
      packages = forAllSystems (pkgs: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "datadiff";
          inherit (cargoPackage) version;
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
          meta = {
            inherit (cargoPackage) description homepage;
            license = with pkgs.lib.licenses; [
              mit
              asl20
            ];
            mainProgram = "datadiff";
          };
        };
      });
    };
}
