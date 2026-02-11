{
  lib,
  stdenv,
  fetchFromGitHub,
  libcap,
}:
stdenv.mkDerivation rec {
  pname = "isolate";
  version = "2.2.1";

  src = fetchFromGitHub {
    owner = "ioi";
    repo = "isolate";
    rev = "v${version}";
    hash = "sha256-haH4fjL3cWayYrpUDwD4hUNlxIoN6MdO3QgAqimi/+c=";
  };

  buildInputs = [libcap];

  # Build only the main binary, skip isolate-cg-keeper (requires systemd) and manpages
  buildPhase = ''
    runHook preBuild

    # Build the isolate binary with correct config path compiled in
    make isolate CONFIGDIR=$out/etc

    # Generate config file with a writable box root for container use
    make default.cf SBINDIR=$out/sbin BOXDIR=/var/local/lib/isolate

    # The upstream default.cf uses "cg_root = auto:/run/isolate/cgroup" which
    # relies on isolate-cg-helper (a systemd service) to write the path.
    # In a container there is no systemd, so set an explicit cgroup v2 path.
    sed -i 's|^cg_root = auto:/run/isolate/cgroup|cg_root = /sys/fs/cgroup/isolate|' default.cf

    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall

    mkdir -p $out/bin $out/sbin $out/etc $out/var/local/lib/isolate

    # Install the main binary
    install -m 755 isolate $out/bin/

    # Install the check script (it's a shell script, not compiled)
    install -m 755 isolate-check-environment $out/bin/

    # Install config file
    install -m 644 default.cf $out/etc/isolate

    runHook postInstall
  '';

  meta = with lib; {
    description = "Sandbox for securely executing untrusted programs";
    homepage = "https://github.com/ioi/isolate";
    license = licenses.gpl2;
    platforms = platforms.linux;
    maintainers = [];
  };
}
