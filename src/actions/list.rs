use crate::{local, planner, remote, Opt};

pub fn run(opt: &Opt) -> anyhow::Result<()> {
    info!("listing backup entries");

    let local_list = local::file_list(opt)?;
    let session = remote::connect(opt)?;
    let remote_list = remote::file_list(opt, &session)?;

    let presence = planner::presence(&local_list, &remote_list);

    println!("found {} files", presence.len());
    for (item, location) in presence {
        println!("- {}: {:?}", item, location);
    }

    Ok(())
}
