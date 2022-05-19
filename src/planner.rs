use crate::{FileList, TimeStamp};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub enum Presence {
    Local,
    Remote,
    LocalAndRemote,
}

/// Check which files exist on remote, local, or both
pub fn presence(local: &FileList, remote: &FileList) -> BTreeMap<TimeStamp, Presence> {
    let mut presence = BTreeMap::new();

    for &file in local.keys() {
        presence.insert(file, Presence::Local);
    }

    for &file in remote.keys() {
        let entry = presence.entry(file).or_insert(Presence::Remote);

        if let Presence::Local = *entry {
            *entry = Presence::LocalAndRemote;
        }
    }

    presence
}

#[derive(Clone, Copy, Debug)]
pub enum TransferKind {
    Full,
    DeltaFrom(TimeStamp),
}

pub struct Plan {
    /// The most recent common file
    pub last_common: Option<TimeStamp>,

    /// The list of planned transfers
    pub transfers: BTreeMap<TimeStamp, TransferKind>,
}

/// For every trailing local file that is not in the remote, return a backup plan.
///
/// The backup plans may depend on each other, so they must be executed in order.
///
/// Set `include_all` to include all local files, not just the most recent.
pub fn plan(local: &FileList, remote: &FileList, include_all: bool) -> Plan {
    let upload_list: BTreeSet<_> = {
        // go through the local files in order, starting with the most recent
        let local = local.keys().rev();

        if include_all {
            // take all files that doesn't exist in the remote
            local.filter(|ts| !remote.contains_key(ts)).collect()
        } else {
            // take only the most recent files that doesn't exist in the remote
            local.take_while(|ts| !remote.contains_key(ts)).collect()
        }
    };

    // find the closest parent file of the first planned upload
    let head_item = upload_list.iter().next().copied();
    let last_common = head_item
        .and_then(|&first| local.range(..first).last())
        .map(|(&entry, _)| entry);
    let head_item_plan = last_common
        .map(TransferKind::DeltaFrom)
        .unwrap_or(TransferKind::Full);

    let tail_items_plan = upload_list
        .iter()
        .skip(1)
        .zip(upload_list.iter())
        .map(|(&&child, &&parent)| (child, TransferKind::DeltaFrom(parent)));

    let transfers = head_item
        .iter()
        .map(|&&head| (head, head_item_plan))
        .chain(tail_items_plan)
        .collect();

    Plan {
        transfers,
        last_common,
    }
}
