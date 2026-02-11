# Minimal JDK built with jlink
#
# Uses jlink to create a stripped-down Java image containing only
# jdk.compiler (javac) and its transitive dependencies (java.base,
# java.compiler, jdk.internal.opt, jdk.zipfs).
#
# This results in a much smaller installation compared to the full
# headless JDK.
{
  lib,
  stdenv,
  jdk21_headless,
}:
stdenv.mkDerivation {
  pname = "jdk-minimal";
  version = jdk21_headless.version;

  dontUnpack = true;

  buildPhase = ''
    runHook preBuild

    ${jdk21_headless}/bin/jlink \
      --add-modules jdk.compiler \
      --output $out \
      --strip-debug \
      --no-man-pages \
      --no-header-files \
      --compress zip-6

    runHook postBuild
  '';

  # jlink creates the full directory structure; no install phase needed
  dontInstall = true;

  meta = with lib; {
    description = "Minimal JDK with javac via jlink (jdk.compiler + java.base)";
    platforms = platforms.linux;
  };
}
