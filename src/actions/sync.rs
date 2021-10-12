use crate::local;
use crate::planner::{self, TransferKind};
use crate::remote;
use crate::Opt;
use ssh2::Session;
use std::io::{self, Read};
use std::process::{Command, Stdio};

pub fn run(opt: &Opt, sync_all: bool) -> anyhow::Result<()> {
    // TODO: currently we only sync the latest local files
    // --all will force a sync of ALL files on local which does not exist on remote
    if sync_all {
        error!("backup --all is not yet implemented");
        unimplemented!();
    }

    info!("generating backup plan");
    let local_list = local::file_list(opt)?;
    let session = remote::connect(opt)?;
    let remote_list = remote::file_list(opt, &session)?;

    let plan = planner::plan(&local_list, &remote_list);

    if plan.transfers.is_empty() {
        info!("nothing to do");
        return Ok(());
    }

    info!("performing backup plan");
    for (item, transfer) in plan.transfers {
        let parent = match transfer {
            TransferKind::DeltaFrom(parent) => Some(
                local_list
                    .get(&parent)
                    .unwrap_or_else(|| &remote_list[&parent])
                    .as_str(),
            ),
            TransferKind::Full => None,
        };

        send_snapshot(opt, &session, &local_list[&item], parent)?;
    }

    Ok(())
}

fn send_snapshot(
    opt: &Opt,
    session: &Session,
    snapshot: &str,
    parent: Option<&str>,
) -> anyhow::Result<()> {
    info!("[{}] transmitting delta", snapshot);

    // spawn btrfs send
    let mut send = Command::new("btrfs")
        .arg("send")
        .args(parent.iter().flat_map(|parent| ["-p", parent]))
        .arg(snapshot)
        .current_dir(&opt.path)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    // capture btrfs send output
    let mut send_stdout = send
        .stdout
        .take()
        .ok_or_else(|| anyhow::format_err!("failed to take stdout"))?;

    // start btrfs receive
    let remote_path = opt
        .remote
        .path
        .to_str()
        .ok_or_else(|| anyhow::format_err!("path not utf-8"))?;
    let mut receive = session.channel_session()?;
    receive.exec(&format!(r#"btrfs receive "{}""#, remote_path,))?;

    // pipe send to receive
    let num_bytes = io::copy(&mut send_stdout, &mut receive)?;
    info!("[{}] sent {} bytes", snapshot, num_bytes);

    // wait for send to complete
    let local_out = send.wait_with_output()?;
    if !local_out.status.success() {
        let stderr = std::str::from_utf8(&local_out.stderr)
            .unwrap_or("failed to parse stderr, not valid utf8");
        anyhow::bail!("btrfs send failed:\n{}", stderr);
    }

    // wait for receive to complete
    receive.send_eof()?;
    let mut remote_err = String::new();
    receive.stderr().read_to_string(&mut remote_err)?;
    receive.wait_close()?;
    let status = receive.exit_status()?;
    if status != 0 {
        anyhow::bail!("btrfs receive failed\nstderr:\n{}", remote_err);
    }

    Ok(())
}
