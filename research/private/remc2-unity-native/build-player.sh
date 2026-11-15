#!/bin/sh
# Ordinary local build: ./build-player.sh [--native-only] [--release]
# Python owns structured build metadata; the shell only locates this project.
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec python3 - "$script_dir" "$@" <<'PY'
import argparse
import ctypes
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

PROJECT = Path(sys.argv[1])
SIM = PROJECT.parent / 'remc2-sim-rs'
PLAYER = 'MagicCarpetNative.x86_64'
PLUGIN = Path('MagicCarpetNative_Data/Plugins/x86_64/libsim_capi.so')
ASSET_PLUGIN = PROJECT / 'Assets/Plugins/x86_64/libsim_capi.so'
INSTALLED = PROJECT / 'Builds/Linux'
RECEIPT = 'build-install.json'
REBUILD = f'Run {PROJECT / "build-player.sh"} for a full build.'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def files_digest(roots, base, excluded=()):
    result = {}
    for root in roots:
        if not root.exists():
            raise RuntimeError(f'Missing build input: {root}. {REBUILD}')
        for path in sorted(root.rglob('*')):
            if path.is_file() and path not in excluded:
                result[str(path.relative_to(base))] = digest(path)
    return result


def unity_inputs():
    return files_digest([PROJECT / name for name in ('Assets', 'Packages', 'ProjectSettings')],
                        PROJECT, (ASSET_PLUGIN,))


def player_inputs(inputs):
    # These C# files compile only into the existing Editor test assembly.
    # Keep its asmdef and all metadata/assets in the compatibility comparison:
    # changing that platform contract must still require a player rebuild.
    test_root = Path('Assets/Tests/EditMode')
    definition = PROJECT / test_root / 'MagicCarpet.Native.EditModeTests.asmdef'
    editor_only = definition.is_file() and (
        json.loads(definition.read_text()).get('includePlatforms') == ['Editor'])
    return {name: value for name, value in inputs.items()
            if not (editor_only and Path(name).parent == test_root and Path(name).suffix == '.cs')}


def native_contract(sim=SIM):
    return files_digest([sim / 'crates/sim-capi/include'], sim)


def rust_inputs(sim=SIM):
    inputs = files_digest([sim / 'crates'], sim)
    for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'):
        inputs[name] = digest(sim / name)
    return inputs


def payload(root):
    return files_digest([root], root, (root / PLUGIN, root / RECEIPT))


def check_install(check_inputs=True, native_source_root=None):
    try:
        receipt = json.loads((INSTALLED / RECEIPT).read_text())
        if receipt['schema'] != 'mc-player-install/v1':
            raise ValueError('unknown install schema')
        if not os.access(INSTALLED / PLAYER, os.X_OK):
            raise ValueError('player executable is missing')
        if receipt['player_files'] != payload(INSTALLED):
            raise ValueError('installed Unity player files changed')
        if receipt['native_sha256'] != digest(INSTALLED / PLUGIN):
            raise ValueError('installed native plugin changed')
        sim = native_source_root or Path(receipt.get('native_source_root', str(SIM)))
        if not sim.is_absolute():
            raise ValueError('native source root is not absolute')
        if check_inputs and (player_inputs(receipt['unity_inputs']) != player_inputs(unity_inputs()) or
                             receipt['native_contract'] != native_contract(sim)):
            raise ValueError('Unity source, settings, packaged assets or native contract changed')
        return receipt
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise RuntimeError(f'Missing or incompatible installed player: {error}. {REBUILD}') from error


def reject_running():
    executable = str(INSTALLED / PLAYER)
    plugin = str(INSTALLED / PLUGIN)
    for process in Path('/proc').iterdir():
        if not process.name.isdigit():
            continue
        try:
            running = os.readlink(process / 'exe').removesuffix(' (deleted)')
            if running == executable or plugin in (process / 'maps').read_text():
                raise RuntimeError(f'Player/plugin is in use by PID {process.name}; '
                                   'quit that player, then build and restart deliberately.')
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue


def atomic_copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    # Same-directory rename never writes into a mapped shared-library inode.
    with tempfile.NamedTemporaryFile(dir=destination.parent, prefix='.native-', delete=False) as stream:
        temporary = Path(stream.name)
    shutil.copy2(source, temporary)
    temporary.replace(destination)


def write_receipt(root, receipt):
    with tempfile.NamedTemporaryFile(mode='w', dir=root, prefix='.install-', delete=False) as stream:
        json.dump(receipt, stream, indent=2, sort_keys=True)
        stream.write('\n')
        temporary = Path(stream.name)
    temporary.replace(root / RECEIPT)


def command(arguments, cwd, log, environment=None, diagnostic_log=None):
    print(f'Log: {log}', flush=True)
    with log.open('w') as stream:
        result = subprocess.run(arguments, cwd=cwd, env=environment,
                                stdout=stream, stderr=subprocess.STDOUT, check=False)
    if result.returncode:
        hint = (' Unity Editor has no active license; sign in and activate it in Unity Hub '
                'or Unity CLI, then rerun this command.'
                if diagnostic_log is not None and result.returncode == 198 else '')
        raise RuntimeError(f'Command failed ({result.returncode}): {arguments[0]}. '
                           f'Read {diagnostic_log or log}; command output: {log}.{hint}')


def main():
    parser = argparse.ArgumentParser(prog=str(PROJECT / 'build-player.sh'), description='Build and install the matching local Linux player. '
        'Use --native-only for Rust-only changes; quit the player before installing. '
        'Logs and previous full builds remain under Builds/.')
    parser.add_argument('--native-only', action='store_true', help='reuse a compatible Unity player')
    parser.add_argument('--native-source-root', type=Path,
                        help='Rust workspace to build and capture (default: sibling remc2-sim-rs)')
    native_profile = parser.add_mutually_exclusive_group()
    native_profile.add_argument('--release', action='store_true', help='use optimized Cargo artifacts (the default)')
    native_profile.add_argument('--dev', action='store_true', help='use unoptimized native artifacts for debugging')
    parser.add_argument('--native-artifact', type=Path,
                        help='reuse an already verified native artifact instead of invoking Cargo')
    parser.add_argument('--expected-native-sha256',
                        help='required exact SHA256 for --native-artifact')
    parser.add_argument('--check-installed', action='store_true', help='validate installed files and source compatibility')
    parser.add_argument('--launch', action='store_true', help=argparse.SUPPRESS)
    if sys.argv[2:3] == ['--launch']:
        options = parser.parse_args(['--launch'])
        player_args = sys.argv[3:]
    else:
        options, player_args = parser.parse_known_args(sys.argv[2:])
    if player_args and not options.launch:
        parser.error('unrecognized arguments: ' + ' '.join(player_args))
    sim = SIM
    if options.native_source_root:
        if options.launch or options.check_installed:
            parser.error('--native-source-root is only for a build/install')
        sim = options.native_source_root.resolve(strict=True)
        if (not (sim / 'crates/sim-capi/include').is_dir() or
                not all((sim / name).is_file() for name in
                        ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'))):
            raise RuntimeError(f'Invalid native source workspace: {sim}')
    if bool(options.native_artifact) != bool(options.expected_native_sha256):
        parser.error('--native-artifact and --expected-native-sha256 must be supplied together')
    if options.native_artifact:
        if options.release or options.dev or options.launch or options.check_installed:
            parser.error('--native-artifact is only for a build/install, without a Cargo profile option')
        options.native_artifact = options.native_artifact.resolve(strict=True)
        if (not options.native_artifact.is_file() or
                len(options.expected_native_sha256) != 64 or
                digest(options.native_artifact) != options.expected_native_sha256):
            raise RuntimeError('Reused native artifact does not match its required SHA256')
    (PROJECT / 'Builds').mkdir(exist_ok=True)
    with (PROJECT / 'Builds/.player-install.lock').open('a') as lock:
        if options.launch or options.check_installed:
            fcntl.flock(lock, fcntl.LOCK_SH)
            receipt = check_install()
            if options.launch:
                if digest(ASSET_PLUGIN) != receipt['native_sha256']:
                    raise RuntimeError('Installed player and source native plugin differ; '
                                       'finish build-player.sh --native-only or run a full build.')
                # Retain the shared installation lock for the player's entire lifetime.
                os.set_inheritable(lock.fileno(), True)
                print(f'Native plugin SHA256: {receipt["native_sha256"]}', flush=True)
                os.execv(INSTALLED / PLAYER, [str(INSTALLED / PLAYER), *player_args])
            print(json.dumps({'schema': 'mc-player-status/v1', 'player': str(INSTALLED / PLAYER),
                              'native_sha256': receipt['native_sha256']}))
            return
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError('Player or another build is active; quit it before installing.') from error
        reject_running()
        receipt = None
        if options.native_only:
            if options.native_source_root:
                receipt = check_install(native_source_root=sim)
            else:
                receipt = check_install()
                sim = Path(receipt.get('native_source_root', str(SIM)))
        before = unity_inputs()
        contract = native_contract(sim)
        rust_before = rust_inputs(sim)
        version = (PROJECT / 'ProjectSettings/ProjectVersion.txt').read_text().splitlines()[0].split(': ', 1)[1]
        editor = Path(os.environ.get('MC_UNITY_EDITOR', str(Path.home() / f'Unity/Hub/Editor/{version}/Editor/Unity')))
        if not options.native_only and not os.access(editor, os.X_OK):
            raise RuntimeError(f'Unity {version} is missing at {editor}; set MC_UNITY_EDITOR to that editor.')
        logs = Path(tempfile.mkdtemp(prefix='build-', dir=PROJECT / 'Builds'))
        cargo_log = logs / 'cargo.log'
        cargo = ['cargo', '+1.98.0', 'build', '-p', 'sim-capi', '--locked',
                 '--message-format=json-render-diagnostics']
        if not options.dev:
            cargo.append('--release')
        if options.native_artifact:
            artifact = options.native_artifact
            print(f'Reusing verified native artifact: {artifact}; no Cargo invocation.', flush=True)
        else:
            command(cargo, sim, cargo_log)
            artifacts = []
            for line in cargo_log.read_text().splitlines():
                try:
                    message = json.loads(line)
                except ValueError:
                    continue  # Cargo progress and rendered diagnostics share this log.
                if message.get('reason') == 'compiler-artifact' and message.get('target', {}).get('name') == 'sim_capi':
                    artifacts.extend(Path(name) for name in message['filenames'] if name.endswith('/libsim_capi.so'))
            if len(artifacts) != 1 or not artifacts[0].is_file():
                raise RuntimeError(f'Cargo did not produce one sim-capi shared library; read {cargo_log}')
            artifact = artifacts[0]
        library = ctypes.CDLL(str(artifact))
        library.mc_native_abi_version.restype = ctypes.c_uint32
        if library.mc_native_abi_version() != 2:
            raise RuntimeError('Native ABI changed; migrate the Unity consumer before building.')
        native_hash = digest(artifact)
        if options.native_artifact and native_hash != options.expected_native_sha256:
            raise RuntimeError('Reused native artifact changed during capture')
        if before != unity_inputs() or contract != native_contract(sim) or rust_before != rust_inputs(sim):
            raise RuntimeError(f'Build inputs changed during Cargo; retry at a stable source point. Logs: {logs}')
        reject_running()
        atomic_copy(artifact, ASSET_PLUGIN)
        print(f'Native build captured: {native_hash}; Rust source edits may resume.', flush=True)
        if options.native_only:
            # Validate again after Cargo in case something outside the launcher touched the player.
            check_install(native_source_root=sim)
            atomic_copy(artifact, INSTALLED / PLUGIN)
            receipt['native_sha256'] = native_hash
            receipt['cargo_artifact'] = str(artifact)
            receipt['rust_inputs'] = rust_before
            receipt['native_source_root'] = str(sim.resolve())
            receipt['logs'] = str(logs)
            write_receipt(INSTALLED, receipt)
        else:
            stage = logs / 'Linux'
            environment = dict(os.environ, MC_PLAYER_BUILD_OUTPUT=str(stage))
            command([str(editor), '-batchmode', '-nographics', '-quit', '-projectPath', str(PROJECT),
                     '-executeMethod', 'MagicCarpet.Native.Editor.BuildModernPlayer.BuildLinuxPlayer',
                     '-logFile', str(logs / 'unity.log')], PROJECT, logs / 'unity-command.log', environment,
                    diagnostic_log=logs / 'unity.log')
            # Rust source may advance after the checked native artifact was captured.
            # This install records rust_before and still verifies the bundled bytes.
            if before != unity_inputs() or contract != native_contract(sim):
                raise RuntimeError(f'Unity build changed authored inputs or source moved; installed player preserved. Read {logs}')
            if (not os.access(stage / PLAYER, os.X_OK) or not (stage / PLUGIN).is_file()
                    or digest(stage / PLUGIN) != native_hash):
                raise RuntimeError(f'Unity output is missing or bundled a different plugin. Read {logs / "unity.log"}')
            receipt = {'schema': 'mc-player-install/v1', 'unity_inputs': before,
                       'native_contract': contract, 'native_sha256': native_hash,
                       'rust_inputs': rust_before, 'native_source_root': str(sim.resolve()),
                       'cargo_artifact': str(artifact), 'player_files': payload(stage), 'logs': str(logs)}
            write_receipt(stage, receipt)
            reject_running()
            if INSTALLED.exists():
                backup = PROJECT / 'Builds' / f'Linux-before-{logs.name}'
                INSTALLED.rename(backup)
                print(f'Previous player preserved: {backup}', flush=True)
            try:
                stage.rename(INSTALLED)
            except OSError:
                if not INSTALLED.exists() and 'backup' in locals():
                    backup.rename(INSTALLED)
                raise
        check_install()
        print(f'Installed {"native refresh" if options.native_only else "Linux player"}: {INSTALLED / PLAYER}\n'
              f'Native plugin SHA256: {native_hash}\nLogs: {logs}', flush=True)


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, AttributeError, ValueError, IndexError) as error:
        print(f'Build/install failed: {error}', file=sys.stderr)
        sys.exit(1)
PY
