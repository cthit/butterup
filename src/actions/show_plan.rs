use crate::{local, planner, remote, Opt};

pub fn run(opt: &Opt) -> anyhow::Result<()> {
    info!("showing backup plan");

    let local_list = local::file_list(opt)?;
    let session = remote::connect(opt)?;
    let remote_list = remote::file_list(opt, &session)?;

    let plan = planner::plan(&local_list, &remote_list);
    let presence = planner::presence(&local_list, &remote_list);

    println!(
        "found that {} out of {} folders need backup",
        plan.transfers.len(),
        presence.len(),
    );

    if !plan.transfers.is_empty() {
        println!("plan:");
        for (item, item_plan) in plan.transfers {
            println!("- {:?}: {:?}", item, item_plan);
        }
    }

    Ok(())
}
