{
  inputs = {
    dream2nix.url = "github:nix-community/dream2nix";
    src.url = "git+https://git.sr.ht/~proycon/vocage";
    src.flake = false;
  };

  outputs = {
    self,
    dream2nix,
    src,
  }:
    (dream2nix.lib.makeFlakeOutputs {
      systems = ["x86_64-linux"];
      config.projectRoot = ./.;
      source = src;
      projects = ./projects.toml;
    })
    ;
}
