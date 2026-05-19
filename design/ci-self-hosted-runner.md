# Self-hosted Windows runner on Tailscale

Step-by-step setup for the `windows-integration` CI job. Runs `cargo test`
against the full workspace including TSF / IPC / Registry code that the Linux
`engine-tests` job cannot execute.

## Threat model

A self-hosted runner executes whatever the workflow file says to execute. If a
fork PR can trigger the workflow, an attacker can run arbitrary code on the
runner. `windows-integration` therefore gates on
`github.event_name == 'push' || github.event_name == 'merge_group'` so only
post-merge commits and the merge queue ever land on this box.

Treat the runner machine as production-adjacent:

- Dedicated Windows user with no access to personal data.
- No SSH/RDP from outside Tailscale.
- No interactive secrets stored in the user profile (no signed-in browsers).
- Code signing keys live elsewhere (`code-signing.yml` already runs separately).

## One-time machine prep

1. Install Windows 11 (the IDE target platform). Apply updates.
2. Create a local non-admin user `forgejo-runner`. Disable interactive login.
3. Install Rust via `rustup`, plus the GCC toolchain you need for native code:
   ```pwsh
   winget install Rustlang.Rustup
   rustup default stable
   ```
4. Install `git`, `cargo-xtask` prerequisites, and any C build tools the
   chewing data pipeline expects.
5. Install Tailscale, sign in with the **service** account, enable the
   `tag:ci-runner` tag in the admin console. The tag ACL should only allow
   inbound traffic from the Forgejo server's tag.
6. Reboot. Verify `tailscale status` shows the runner online.

## Forgejo runner install

1. Download the matching `forgejo-runner` binary on the runner machine. Place
   it under `C:\forgejo-runner\`.
2. Register the runner with the Forgejo server using an **ephemeral** token
   (Settings → Actions → Runners → Create new runner). Ephemeral means it
   accepts one job and then exits, which is what you want for security.
3. Apply labels so the workflow can target it:
   ```yaml
   labels: ["self-hosted", "windows", "x86_64"]
   ```
4. Wrap the runner in a Windows Service so it survives reboot. Either
   `nssm install` or the runner's `--install` flag (depending on version).
   Run the service as the `forgejo-runner` user.
5. Configure the runner config to bind to its Tailscale IP only:
   - `bind_address`: the runner's `100.x.y.z` tailnet address.
   - Firewall rule: block inbound from anything except `100.x.0.0/16`.

## Tailscale ACL (server side)

In the Tailscale admin console:

```jsonc
{
  "tagOwners": {
    "tag:ci-server":  ["autogroup:admin"],
    "tag:ci-runner": ["autogroup:admin"]
  },
  "acls": [
    // Forgejo server → runners over Forgejo runner protocol port.
    { "action": "accept",
      "src":    ["tag:ci-server"],
      "dst":    ["tag:ci-runner:*"] },

    // Deny everything else by default.
  ]
}
```

The runner machine has **no inbound public exposure**. All control traffic
travels over the tailnet.

## Forgejo variables

The workflow expects a `CHEWING_PATCH_REPO` variable while the local
libchewing patch is in place. Set it in **Settings → Variables**, not Secrets
— the value is a public git URL, not a credential:

| Variable             | Example value                                      |
| -------------------- | -------------------------------------------------- |
| `CHEWING_PATCH_REPO` | `https://codeberg.org/<you>/libchewing`            |
| `CHEWING_PATCH_REF`  | `fix/fuzzy-partial-prefix-select-with-down`        |

Remove these once the upstream PR merges and the `[patch.crates-io]` block
disappears from `Cargo.toml`.

## Smoke test the setup

After registering the runner, push a no-op commit to a throwaway branch and
verify:

1. `engine-tests` runs on `fedora-latest` and passes.
2. `windows-integration` is queued, picked up by the Windows runner, and
   exits 0.
3. `build` (existing job) still produces the MSI.

If `windows-integration` does not start: check the `if:` gate excludes
`pull_request` events. Push a commit directly to a non-default branch and
re-check.

## Operational notes

- Cache: `Swatinem/rust-cache` works on Windows. Cache hits are per-runner,
  so the first run is slow.
- Logs: forgejo-runner writes to its config directory. Rotate weekly.
- Updates: keep rust toolchain and the runner binary on the same major
  version as the Linux runners; otherwise reproducibility suffers.
- Reboot policy: schedule monthly OS updates for off-hours. The Windows
  Service auto-restarts the runner.
