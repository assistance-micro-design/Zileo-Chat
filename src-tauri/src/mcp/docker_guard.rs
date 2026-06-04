// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Docker spawn-argument guard.
//!
//! The MCP user supplies the *entire* docker command line; `build_command`
//! passes `config.args` 100% raw to `docker`. Exec is direct (no shell), so
//! shell injection is not the threat — **container-isolation escape** is.
//!
//! This module exposes a single pure function,
//! [`validate_docker_spawn_args`], wired at the spawn choke-point
//! (`server_handle::build_command`) so it covers every vector that reaches a
//! process spawn — boot (`load_from_db`), create, update, import — and refuses
//! a legacy persisted config at the next boot by construction (never
//! grandfathered).
//!
//! Strategy = **deny-by-default on evasion** (blocklist of escape flags) +
//! **validation of the mount source** (system-path prefixes refused, the
//! docker socket refused), rather than a rigid path allowlist that would break
//! the nominal filesystem MCP server.

use crate::models::mcp::MCPDeploymentMethod;

/// System path prefixes whose directory **and every descendant** are refused
/// as a bind-mount source (`/etc` and `/etc/anything` are both refused). `/`
/// and `/home` are handled separately (see [`is_forbidden_source`]).
const FORBIDDEN_SOURCE_PREFIXES: &[&str] = &[
    "/etc", "/dev", "/proc", "/sys", "/boot", "/root", "/var", "/run", "/usr", "/bin", "/sbin",
    "/lib", "/lib64", "/opt",
];

/// Validates the raw argument vector that will be handed to `docker`.
///
/// Only [`MCPDeploymentMethod::Docker`] is guarded here: `npx`/`uvx`
/// supply-chain trust is a separate concern and `http` never
/// reaches a process spawn, so both pass through unchanged.
///
/// # Arguments
/// * `method` - The deployment method of the server being spawned.
/// * `args` - The raw argument vector (`config.args`), e.g.
///   `["run", "-i", "-v", "/data:/d", "image:tag"]`.
///
/// # Returns
/// `Ok(())` when the invocation is considered safe.
///
/// # Errors
/// Returns a human-readable reason (suitable for a form error / structured
/// log) when the invocation would break container isolation: a forbidden
/// global flag, a non-`run` subcommand, a forbidden mount source, or an
/// evasion flag.
pub fn validate_docker_spawn_args(
    method: &MCPDeploymentMethod,
    args: &[String],
) -> Result<(), String> {
    if *method != MCPDeploymentMethod::Docker {
        return Ok(());
    }

    // [P0] Subcommand must be `run` or `exec` (optionally `container run`/
    // `container exec`); no global flag may precede it. `docker -H
    // tcp://attacker:2375 run …` would target a remote daemon and bypass the
    // sandbox entirely.
    let (subcommand, sub_idx) = match args.first().map(String::as_str) {
        Some("run") => (Subcommand::Run, 0),
        Some("exec") => (Subcommand::Exec, 0),
        Some("container") => match args.get(1).map(String::as_str) {
            Some("run") => (Subcommand::Run, 1),
            Some("exec") => (Subcommand::Exec, 1),
            _ => return Err(unsupported_subcommand_err(args.get(1).map(String::as_str))),
        },
        Some(other) => return Err(unsupported_subcommand_err(Some(other))),
        None => return Ok(()), // empty args is caught earlier by build_command
    };

    let tokens = &args[sub_idx + 1..];
    match subcommand {
        Subcommand::Run => validate_run_tokens(tokens),
        Subcommand::Exec => validate_exec_tokens(tokens),
    }
}

/// The docker subcommands accepted for an MCP stdio server.
enum Subcommand {
    /// `docker run` — starts a container; full mount + evasion blocklist.
    Run,
    /// `docker exec` — runs a command in an already-started container; reduced
    /// blocklist (no mounts/network/devices are possible on `exec`).
    Exec,
}

/// Error for an args vector whose subcommand is neither `run` nor `exec`.
fn unsupported_subcommand_err(got: Option<&str>) -> String {
    format!(
        "docker: only the 'run' or 'exec' subcommand is allowed (got '{}'); \
         global flags before the subcommand are refused",
        got.unwrap_or("")
    )
}

/// Validates the tokens after `run`: mounts + the full evasion blocklist.
fn validate_run_tokens(tokens: &[String]) -> Result<(), String> {
    let mut i = 0;
    while i < tokens.len() {
        let tok = tokens[i].as_str();

        // --- mount flags taking the spec as the NEXT token ---
        if tok == "-v" || tok == "--volume" {
            let spec = tokens.get(i + 1).ok_or_else(|| missing_value(tok))?;
            validate_short_volume(spec)?;
            i += 2;
            continue;
        }
        if tok == "--mount" {
            let spec = tokens.get(i + 1).ok_or_else(|| missing_value(tok))?;
            validate_mount_spec(spec)?;
            i += 2;
            continue;
        }

        // --- mount flags with an inline value (`-v=…`, `--volume=…`, `--mount=…`) ---
        if let Some(spec) = tok
            .strip_prefix("-v=")
            .or_else(|| tok.strip_prefix("--volume="))
        {
            validate_short_volume(spec)?;
            i += 1;
            continue;
        }
        if let Some(spec) = tok.strip_prefix("--mount=") {
            validate_mount_spec(spec)?;
            i += 1;
            continue;
        }

        // --- glued short volume (`-v<spec>` without a space or `=`, e.g.
        // `-v/etc:/x`). pflag accepts the value stuck to the short flag, so
        // this must be screened just like `-v <spec>` (HARDENING P0-A). The
        // exact `-v` and `-v=` forms were already handled above; `--volume`
        // starts with `--` and is not matched here.
        if let Some(spec) = tok.strip_prefix("-v") {
            if !spec.is_empty() && !spec.starts_with('=') {
                validate_short_volume(spec)?;
                i += 1;
                continue;
            }
        }

        // --- evasion / restricted flags ---
        check_forbidden_flag(tok, tokens.get(i + 1).map(String::as_str))?;

        // --- grouped short flags (e.g. `-it` => `-i` + `-t`) ---
        if is_grouped_short(tok) && tok.contains('v') {
            return Err(format!(
                "docker: ambiguous grouped short flag '{tok}' contains '-v'; \
                 pass the volume flag separately"
            ));
        }

        i += 1;
    }

    Ok(())
}

/// Validates the tokens after `exec`. `docker exec` cannot mount volumes or
/// touch the network/devices, so its only dangerous flags are `--privileged`
/// (extended privileges in the target container) and `--env-file` (reads an
/// arbitrary host file). Everything else (`-i`/`-t`/`-d`/`-e KEY=VAL`/`-w`/
/// `-u`/container/command) is benign and passes.
fn validate_exec_tokens(tokens: &[String]) -> Result<(), String> {
    for tok in tokens {
        let name = tok.split_once('=').map(|(n, _)| n).unwrap_or(tok.as_str());
        match name {
            "--privileged" => {
                return Err("docker exec: '--privileged' is refused".to_string());
            }
            "--env-file" => {
                return Err("docker exec: '--env-file' reads an arbitrary host file".to_string());
            }
            _ => {}
        }
    }
    Ok(())
}

/// Returns true for a cluster of short flags like `-it` (single leading dash,
/// at least two ASCII-alphabetic chars, no `=`).
fn is_grouped_short(tok: &str) -> bool {
    let bytes = tok.as_bytes();
    tok.len() >= 3
        && bytes[0] == b'-'
        && bytes[1] != b'-'
        && tok[1..].chars().all(|c| c.is_ascii_alphabetic())
}

fn missing_value(flag: &str) -> String {
    format!("docker: flag '{flag}' is missing its value")
}

/// Validates the value of a forbidden/restricted long flag. `next` is the
/// following token (used when the value is space-separated, e.g.
/// `--security-opt seccomp=unconfined`).
fn check_forbidden_flag(tok: &str, next: Option<&str>) -> Result<(), String> {
    // Split an inline `--flag=value`.
    let (name, inline) = match tok.split_once('=') {
        Some((n, v)) => (n, Some(v)),
        None => (tok, None),
    };

    // Resolve the effective value: inline first, else the next token.
    let value = || inline.or(next).unwrap_or("");

    match name {
        // Unconditional refusals.
        "--privileged" => Err("docker: '--privileged' grants full host access".into()),
        "--device" => Err("docker: '--device' exposes host devices".into()),
        "--device-cgroup-rule" => Err("docker: '--device-cgroup-rule' is refused".into()),
        "--cap-add" => Err("docker: '--cap-add' is refused (no capabilities may be added)".into()),
        "--add-host" => Err("docker: '--add-host' is refused".into()),
        "--gpus" => Err("docker: '--gpus' exposes host GPU devices".into()),
        "--env-file" => Err("docker: '--env-file' reads an arbitrary host file".into()),
        "--volumes-from" => Err("docker: '--volumes-from' is refused".into()),
        "--runtime" => Err("docker: '--runtime' is refused".into()),
        // P3 (spec l.666): files written to / read from arbitrary host paths,
        // and flags that tune host kernel / cgroup / group context.
        "--cidfile" => Err("docker: '--cidfile' writes to an arbitrary host path".into()),
        "--pidfile" => Err("docker: '--pidfile' writes to an arbitrary host path".into()),
        "--label-file" => Err("docker: '--label-file' reads an arbitrary host file".into()),
        "--sysctl" => Err("docker: '--sysctl' tunes host kernel parameters".into()),
        "--cgroup-parent" => {
            Err("docker: '--cgroup-parent' places the container in a host cgroup".into())
        }
        "--group-add" => Err("docker: '--group-add' adds host groups to the container".into()),

        // Global flags that must never appear before `run` (enforced by the
        // subcommand check); refused here too as defense in depth. `-l` is
        // intentionally absent: after `run` it is `--label` (benign), and the
        // global short `-l` before `run` is already blocked by the run-index
        // check above.
        "-H" | "--host" | "--context" | "--config" | "-D" | "--debug" | "--log-level" => {
            Err(format!("docker: global flag '{name}' is refused"))
        }
        n if n.starts_with("--tls") => Err(format!("docker: global TLS flag '{name}' is refused")),

        // --security-opt: refuse unconfined / disabled / custom seccomp profiles.
        "--security-opt" => {
            let v = value();
            if v.contains("unconfined")
                || v.starts_with("label=disable")
                || (v.starts_with("seccomp=") && v != "seccomp=default")
            {
                Err(format!("docker: '--security-opt {v}' weakens isolation"))
            } else {
                Ok(())
            }
        }

        // Namespace sharing with the host or another container.
        "--pid" | "--ipc" | "--uts" | "--userns" | "--cgroupns" => {
            let v = value();
            if v == "host" || v.starts_with("container:") {
                Err(format!(
                    "docker: '{name}={v}' shares a host/container namespace"
                ))
            } else {
                Ok(())
            }
        }

        // Host networking.
        "--network" | "--net" => {
            let v = value();
            if v == "host" || v.starts_with("container:") {
                Err(format!(
                    "docker: '{name}={v}' shares the host network namespace"
                ))
            } else {
                Ok(())
            }
        }

        _ => Ok(()),
    }
}

/// Validates a `-v` / `--volume` spec (`src:dst[:mode]`, or a single field for
/// an anonymous/named volume).
fn validate_short_volume(spec: &str) -> Result<(), String> {
    let fields: Vec<&str> = spec.split(':').collect();
    // A single field is an anonymous volume at a container path (no host
    // source) — safe.
    if fields.len() < 2 {
        return Ok(());
    }
    // The mode suffix (`:ro`/`:rw`/`:z`/…) lives in later fields; the source is
    // always the first field, so taking field[0] already ignores the mode.
    validate_mount_source(fields[0])
}

/// Validates a `--mount type=…,source=…,target=…` spec.
fn validate_mount_spec(spec: &str) -> Result<(), String> {
    let mut mount_type = "volume"; // docker's default when `type=` is omitted
    let mut source: Option<&str> = None;
    // HARDENING P0-B: the `local` volume driver can bind an arbitrary host path
    // via `volume-opt=device=<path>` (with `o=bind`), bypassing a `type=bind`
    // source check. Capture the device path and screen it like a bind source.
    let mut device: Option<&str> = None;
    for part in spec.split(',') {
        if let Some(v) = part.strip_prefix("type=") {
            mount_type = v;
        } else if let Some(v) = part.strip_prefix("source=") {
            source = Some(v);
        } else if let Some(v) = part.strip_prefix("src=") {
            source = Some(v);
        } else if let Some(v) = part.strip_prefix("volume-opt=device=") {
            device = Some(v);
        }
    }
    // A `volume-opt=device=<path>` exposes a host path regardless of `type`.
    if let Some(dev) = device {
        validate_mount_source(dev)?;
    }
    match mount_type {
        // bind => the source is a host path and must be validated.
        "bind" => match source {
            Some(s) => validate_mount_source(s),
            None => Err("docker: '--mount type=bind' without a source is refused".into()),
        },
        // tmpfs / volume (named) => no host path is exposed (the `device`
        // host-path escape, if any, was already screened above).
        _ => Ok(()),
    }
}

/// Validates a bind-mount **source**: rejects host system paths and the docker
/// socket; allows named volumes and data sub-directories.
fn validate_mount_source(source: &str) -> Result<(), String> {
    // Unresolved environment variables cannot be classified safely.
    if source.contains('$') {
        return Err(format!(
            "docker: mount source '{source}' contains an unresolved environment variable"
        ));
    }
    // `~` is not expanded; require an absolute path instead.
    if source.starts_with('~') {
        return Err(format!(
            "docker: mount source '{source}' uses '~'; pass an absolute path"
        ));
    }
    // A bare name (no '/') is a named volume — always allowed.
    if !source.contains('/') {
        return Ok(());
    }
    // A relative host path resolves against the daemon CWD (ambiguous) — refuse.
    if !source.starts_with('/') {
        return Err(format!(
            "docker: mount source '{source}' must be an absolute path or a named volume"
        ));
    }

    let normalized = normalize_abs_path(source)?;
    if is_forbidden_source(&normalized) {
        return Err(format!(
            "docker: mount source '{source}' resolves to the forbidden system path '{normalized}'"
        ));
    }

    // Defense in depth: when the source actually exists, also classify its
    // canonical form (resolves symlinks). Non-existent sources (the common
    // case for `-v`-created dirs) keep only the lexical result above.
    if let Ok(canon) = std::fs::canonicalize(source) {
        if let Some(canon_str) = canon.to_str() {
            if is_forbidden_source(canon_str) {
                return Err(format!(
                    "docker: mount source '{source}' canonicalizes to the forbidden path '{canon_str}'"
                ));
            }
        }
    }

    Ok(())
}

/// Lexically normalizes an absolute path: collapses `//` and `.`, and **hard
/// refuses** any `..` segment (the only directory-escape vector). Never
/// prefix-matches on the raw string.
fn normalize_abs_path(path: &str) -> Result<String, String> {
    let mut segments: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => continue, // collapse `//`, leading/trailing `/`, and `.`
            ".." => {
                return Err(format!(
                    "docker: mount source '{path}' contains a '..' path-escape"
                ));
            }
            s => segments.push(s),
        }
    }
    Ok(format!("/{}", segments.join("/")))
}

/// Classifies a lexically-normalized absolute path as a forbidden mount source.
fn is_forbidden_source(normalized: &str) -> bool {
    // Mounting the host root.
    if normalized == "/" {
        return true;
    }
    // `/home`: refuse the root and a bare user home (`/home/<user>`), but allow
    // a data sub-directory (`/home/<user>/data`).
    if normalized == "/home" {
        return true;
    }
    if let Some(rest) = normalized.strip_prefix("/home/") {
        if !rest.contains('/') {
            return true; // exactly `/home/<user>`
        }
        return false; // deeper -> allowed
    }
    // System prefixes: refuse the dir and all descendants.
    FORBIDDEN_SOURCE_PREFIXES
        .iter()
        .any(|p| normalized == *p || normalized.starts_with(&format!("{p}/")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Validate a docker invocation from a slice of `&str` args.
    fn docker(v: &[&str]) -> Result<(), String> {
        validate_docker_spawn_args(&MCPDeploymentMethod::Docker, &a(v))
    }

    // ---------- REFUS: mount sources ----------

    #[test]
    fn refuse_mount_root() {
        assert!(docker(&["run", "-v", "/:/host", "img"]).is_err());
    }

    // ---------- HARDENING P0-A: glued short volume (`-v<spec>` without `=`) ----------

    #[test]
    fn refuse_glued_short_volume_etc() {
        assert!(docker(&["run", "-v/etc:/x", "img"]).is_err());
    }

    #[test]
    fn refuse_glued_short_volume_docker_sock() {
        assert!(docker(&["run", "-v/var/run/docker.sock:/x", "img"]).is_err());
    }

    #[test]
    fn refuse_glued_short_volume_usr_bin_with_mode() {
        assert!(docker(&["run", "-v/usr/bin:/x:rw", "img"]).is_err());
    }

    #[test]
    fn allow_glued_short_volume_data() {
        // A glued bind to an allowed data dir must still pass (matrix coherence).
        let r = docker(&["run", "-v/home/user/data:/d", "img"]);
        assert!(r.is_ok(), "expected OK, got {r:?}");
    }

    // ---------- HARDENING P0-B: `local` driver bind via volume-opt=device ----------

    #[test]
    fn refuse_mount_volume_opt_device_etc() {
        assert!(docker(&[
            "run",
            "--mount",
            "type=volume,target=/x,volume-opt=type=none,volume-opt=o=bind,volume-opt=device=/etc",
            "img"
        ])
        .is_err());
    }

    #[test]
    fn refuse_mount_volume_opt_device_docker_sock() {
        assert!(docker(&[
            "run",
            "--mount",
            "type=volume,volume-opt=device=/var/run/docker.sock",
            "img"
        ])
        .is_err());
    }

    #[test]
    fn allow_mount_named_volume_simple() {
        // A plain named volume (no device) stays allowed.
        assert!(docker(&[
            "run",
            "--mount",
            "type=volume,source=namedvol,target=/x",
            "img"
        ])
        .is_ok());
    }

    // ---------- coverage: flags the guard already blocks ----------

    #[test]
    fn refuse_device_cgroup_rule() {
        assert!(docker(&["run", "--device-cgroup-rule", "c 1:1 rwm", "img"]).is_err());
    }

    #[test]
    fn refuse_add_host() {
        assert!(docker(&["run", "--add-host", "host:1.2.3.4", "img"]).is_err());
    }

    // ---------- P3 (spec l.666): additional refused flags ----------

    #[test]
    fn refuse_cidfile() {
        // Writes the container id to an arbitrary host path (clobber).
        assert!(docker(&["run", "--cidfile", "/host/cid", "img"]).is_err());
        assert!(docker(&["run", "--cidfile=/host/cid", "img"]).is_err());
    }

    #[test]
    fn refuse_pidfile() {
        assert!(docker(&["run", "--pidfile", "/host/pid", "img"]).is_err());
    }

    #[test]
    fn refuse_label_file() {
        // Reads an arbitrary host file.
        assert!(docker(&["run", "--label-file", "/host/labels", "img"]).is_err());
    }

    #[test]
    fn refuse_sysctl() {
        assert!(docker(&["run", "--sysctl", "net.ipv4.ip_forward=1", "img"]).is_err());
    }

    #[test]
    fn refuse_cgroup_parent() {
        assert!(docker(&["run", "--cgroup-parent", "/host-cgroup", "img"]).is_err());
    }

    #[test]
    fn refuse_group_add() {
        assert!(docker(&["run", "--group-add", "docker", "img"]).is_err());
    }

    #[test]
    fn allow_run_without_the_new_blocked_flags() {
        // A nominal run using only benign flags (none of the new blocklist) still
        // passes — the additions must not over-reject.
        let r = docker(&["run", "-i", "--rm", "-e", "KEY=VAL", "img:tag"]);
        assert!(r.is_ok(), "expected OK, got {r:?}");
    }

    // ---------- REGRESSION: `docker exec` is a legitimate subcommand ----------

    #[test]
    fn allow_exec_interactive() {
        // The slides-tools regression: `docker exec -i <container> <cmd>`.
        assert!(docker(&["exec", "-i", "my-container", "mcp-server"]).is_ok());
    }

    #[test]
    fn allow_exec_grouped_it_shell() {
        assert!(docker(&["exec", "-it", "container", "sh"]).is_ok());
    }

    #[test]
    fn allow_exec_env_and_workdir() {
        assert!(docker(&["exec", "-e", "KEY=VAL", "container", "cmd"]).is_ok());
        assert!(docker(&["exec", "-w", "/app", "container", "cmd"]).is_ok());
    }

    #[test]
    fn allow_container_exec_form() {
        assert!(docker(&["container", "exec", "c", "cmd"]).is_ok());
    }

    #[test]
    fn refuse_exec_privileged() {
        assert!(docker(&["exec", "--privileged", "container", "cmd"]).is_err());
    }

    #[test]
    fn refuse_exec_env_file() {
        assert!(docker(&["exec", "--env-file", "/etc/shadow", "container", "cmd"]).is_err());
        assert!(docker(&["exec", "--env-file=/etc/shadow", "container", "cmd"]).is_err());
    }

    #[test]
    fn refuse_exec_global_flag_before_subcommand() {
        assert!(docker(&["-H", "tcp://x:2375", "exec", "container", "cmd"]).is_err());
    }

    #[test]
    fn refuse_unsupported_subcommand_lists_run_and_exec() {
        let err = docker(&["pull", "img"]).unwrap_err();
        assert!(err.contains("run"), "msg should list run: {err}");
        assert!(err.contains("exec"), "msg should list exec: {err}");
    }

    #[test]
    fn refuse_mount_etc_inline_with_mode() {
        // -v=/etc:/x:ro — inline value + mode suffix must be stripped.
        assert!(docker(&["run", "-v=/etc:/x:ro", "img"]).is_err());
    }

    #[test]
    fn refuse_mount_usr_bin_volume_long() {
        assert!(docker(&["run", "--volume", "/usr/bin:/x:rw", "img"]).is_err());
    }

    #[test]
    fn refuse_mount_docker_sock_via_mount_bind() {
        assert!(docker(&[
            "run",
            "--mount",
            "type=bind,source=/var/run/docker.sock,target=/x",
            "img"
        ])
        .is_err());
    }

    #[test]
    fn refuse_mount_etc_via_mount_src_alias() {
        assert!(docker(&["run", "--mount", "type=bind,src=/etc,target=/x", "img"]).is_err());
    }

    #[test]
    fn refuse_mount_double_slash_sock() {
        assert!(docker(&["run", "-v", "//var/run//docker.sock:/x", "img"]).is_err());
    }

    #[test]
    fn refuse_mount_dotdot_escape_sock() {
        assert!(docker(&["run", "-v", "/var/run/../run/docker.sock:/x", "img"]).is_err());
    }

    // ---------- REFUS: evasion flags ----------

    #[test]
    fn refuse_privileged() {
        assert!(docker(&["run", "--privileged", "img"]).is_err());
    }

    #[test]
    fn refuse_security_opt_seccomp_unconfined() {
        assert!(docker(&["run", "--security-opt", "seccomp=unconfined", "img"]).is_err());
    }

    #[test]
    fn refuse_security_opt_apparmor_unconfined() {
        assert!(docker(&["run", "--security-opt", "apparmor=unconfined", "img"]).is_err());
    }

    #[test]
    fn refuse_device() {
        assert!(docker(&["run", "--device", "/dev/mem", "img"]).is_err());
    }

    #[test]
    fn refuse_pid_host() {
        assert!(docker(&["run", "--pid=host", "img"]).is_err());
    }

    #[test]
    fn refuse_ipc_container() {
        assert!(docker(&["run", "--ipc=container:victim", "img"]).is_err());
    }

    #[test]
    fn refuse_network_host_long() {
        assert!(docker(&["run", "--network=host", "img"]).is_err());
    }

    #[test]
    fn refuse_net_host_alias() {
        assert!(docker(&["run", "--net=host", "img"]).is_err());
    }

    #[test]
    fn refuse_cap_add() {
        assert!(docker(&["run", "--cap-add", "SYS_ADMIN", "img"]).is_err());
    }

    #[test]
    fn refuse_gpus() {
        assert!(docker(&["run", "--gpus", "all", "img"]).is_err());
    }

    #[test]
    fn refuse_env_file() {
        assert!(docker(&["run", "--env-file", "/etc/shadow", "img"]).is_err());
    }

    #[test]
    fn refuse_volumes_from() {
        assert!(docker(&["run", "--volumes-from", "other", "img"]).is_err());
    }

    #[test]
    fn refuse_runtime() {
        assert!(docker(&["run", "--runtime=evil", "img"]).is_err());
    }

    // ---------- REFUS: global flags before subcommand / wrong subcommand ----------

    #[test]
    fn refuse_global_host_flag() {
        assert!(docker(&["-H", "tcp://x:2375", "run", "img"]).is_err());
    }

    #[test]
    fn refuse_global_context_flag() {
        assert!(docker(&["--context", "evil", "run", "img"]).is_err());
    }

    #[test]
    fn refuse_global_config_flag() {
        assert!(docker(&["--config", "/tmp", "run", "img"]).is_err());
    }

    #[test]
    fn refuse_non_run_subcommand() {
        assert!(docker(&["pull", "img"]).is_err());
    }

    // ---------- OK: nominal invocations ----------

    #[test]
    fn allow_home_user_data_bind() {
        assert!(docker(&["run", "-i", "-v", "/home/user/data:/data", "image:tag"]).is_ok());
    }

    #[test]
    fn allow_srv_data_bind_grouped_it() {
        let r = docker(&[
            "run",
            "--rm",
            "-it",
            "-v",
            "/srv/mcp/data:/data",
            "image:tag",
            "arg1",
        ]);
        assert!(r.is_ok(), "expected OK, got {r:?}");
    }

    #[test]
    fn allow_network_none_and_env_with_path_value() {
        let r = docker(&[
            "run",
            "-i",
            "--network=none",
            "-e",
            "SECRET=/path/in/value",
            "image",
        ]);
        assert!(r.is_ok(), "expected OK, got {r:?}");
    }

    #[test]
    fn allow_tmpfs_mount() {
        assert!(docker(&["run", "--mount", "type=tmpfs,target=/tmp", "image"]).is_ok());
    }

    #[test]
    fn allow_cap_drop() {
        assert!(docker(&["run", "--cap-drop", "ALL", "image"]).is_ok());
    }

    #[test]
    fn allow_entrypoint() {
        assert!(docker(&["run", "--entrypoint", "/bin/sh", "image"]).is_ok());
    }

    #[test]
    fn allow_security_opt_no_new_privileges() {
        assert!(docker(&["run", "--security-opt", "no-new-privileges", "image"]).is_ok());
    }

    #[test]
    fn allow_run_alone() {
        assert!(docker(&["run"]).is_ok());
    }

    #[test]
    fn allow_container_run_form() {
        assert!(docker(&["container", "run", "img"]).is_ok());
    }

    #[test]
    fn allow_named_volume() {
        assert!(docker(&["run", "-v", "myvol:/data", "image"]).is_ok());
    }

    // ---------- pass-through: non-docker ----------

    #[test]
    fn passthrough_npx() {
        let args = a(&["-y", "@scope/pkg@1.2.3"]);
        assert!(validate_docker_spawn_args(&MCPDeploymentMethod::Npx, &args).is_ok());
    }

    #[test]
    fn passthrough_uvx() {
        let args = a(&["package-name"]);
        assert!(validate_docker_spawn_args(&MCPDeploymentMethod::Uvx, &args).is_ok());
    }
}
