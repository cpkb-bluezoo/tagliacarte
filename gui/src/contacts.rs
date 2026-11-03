//! The contacts database and its address-book repositories, for the Contacts
//! tab. Reading it is a sequence of events (`contactsStarted`, a repository,
//! group, group target, group member or contact each, `contactsLoaded`), so a
//! large address book is never one big document. Changes are small commands
//! and the page asks for the sequence again afterwards.

use std::sync::Arc;

use tagliacarte_app::frb_api::frb_contacts::{
    FrbContactRepositoryRow, FrbGroupUpsert, FrbRepositoryUpsert, frb_contacts_apply_group_repository_rules,
    frb_contacts_carddav_pull, frb_contacts_carddav_push, frb_contacts_delete, frb_contacts_export_vcard,
    frb_contacts_group_add_member, frb_contacts_group_delete, frb_contacts_group_members_list,
    frb_contacts_group_remove_member, frb_contacts_group_repository_targets_list, frb_contacts_group_upsert,
    frb_contacts_groups_list, frb_contacts_import_vcard_bytes, frb_contacts_list_compact,
    frb_contacts_repositories_list, frb_contacts_repository_delete, frb_contacts_repository_links_for_contact,
    frb_contacts_repository_upsert, frb_contacts_set_group_repository_rule, frb_contacts_set_repository_membership,
    frb_contacts_validate_external_sharing,
};
use tagliacarte_core::json::{JsonWriter, writer_into_string};

use crate::args::Args;
use crate::commands::Ctx;
use crate::events::{flag, int, opt_str, str};

/// The most contacts listed; the database itself clamps to 10 000.
const CONTACT_LIMIT: i64 = 8000;

fn event(build: impl FnOnce(&mut JsonWriter)) -> String {
    let mut w = JsonWriter::new();
    w.write_start_object();
    build(&mut w);
    w.write_end_object();
    writer_into_string(w)
}

fn repository_event(r: &FrbContactRepositoryRow) -> String {
    event(|w| {
        str(w, "type", "contactsRepository");
        int(w, "id", r.id);
        str(w, "name", &r.name);
        str(w, "kind", &r.kind);
        flag(w, "enabled", r.enabled);
        str(w, "baseUrl", &r.base_url);
        str(w, "collectionPath", &r.collection_path);
        flag(w, "defaultNewContact", r.default_new_contact);
        str(w, "syncError", &r.sync_error);
    })
}

/// The whole address book, as events. Everything between `contactsStarted` and
/// `contactsLoaded` replaces what the page held before.
fn load(ctx: &Ctx) -> Result<(), String> {
    ctx.hub.sequence(|| load_events(ctx))
}

fn load_events(ctx: &Ctx) -> Result<(), String> {
    let push = |e: String| ctx.hub.push(e);
    push(event(|w| str(w, "type", "contactsStarted")));
    for r in frb_contacts_repositories_list()? {
        push(repository_event(&r));
    }
    let groups = frb_contacts_groups_list()?;
    for g in &groups {
        push(event(|w| {
            str(w, "type", "contactsGroup");
            int(w, "id", g.id);
            str(w, "name", &g.name);
        }));
    }
    for t in frb_contacts_group_repository_targets_list()? {
        push(event(|w| {
            str(w, "type", "contactsTarget");
            int(w, "groupId", t.group_id);
            int(w, "repositoryId", t.repository_id);
        }));
    }
    for g in &groups {
        for m in frb_contacts_group_members_list(g.id)? {
            push(event(|w| {
                str(w, "type", "contactsMember");
                int(w, "groupId", g.id);
                int(w, "id", m.id);
                str(w, "displayName", &m.display_name);
                opt_str(w, "primaryEmail", &m.primary_email);
            }));
        }
    }
    for c in frb_contacts_list_compact(CONTACT_LIMIT)? {
        push(event(|w| {
            str(w, "type", "contactsContact");
            int(w, "id", c.id);
            str(w, "displayName", &c.display_name);
            flag(w, "externallyShareOk", c.externally_share_ok);
            str(w, "importOrigin", &c.import_origin);
            opt_str(w, "primaryEmail", &c.primary_email);
        }));
    }
    push(event(|w| str(w, "type", "contactsLoaded")));
    Ok(())
}

/// The sync targets of one contact, as events ending in `contactsRepoLinksDone`.
fn repo_links(ctx: &Ctx, contact_id: i64) -> Result<(), String> {
    for l in frb_contacts_repository_links_for_contact(contact_id)? {
        ctx.hub.push(event(|w| {
            str(w, "type", "contactsRepoLink");
            int(w, "contactId", contact_id);
            int(w, "repositoryId", l.repository_id);
            str(w, "name", &l.name);
            str(w, "kind", &l.kind);
            flag(w, "linked", l.linked);
            flag(w, "localDirty", l.local_dirty);
        }));
    }
    ctx.hub.push(event(|w| {
        str(w, "type", "contactsRepoLinksDone");
        int(w, "contactId", contact_id);
    }));
    Ok(())
}

/// The upsert for a repository the page edited. Editing keeps the credential
/// key already stored (the database would blank it if it were left out).
fn repository_upsert(existing: Option<&FrbContactRepositoryRow>, a: &Args) -> Result<FrbRepositoryUpsert, String> {
    let name = a.str("name")?;
    Ok(FrbRepositoryUpsert {
        id: existing.map(|e| e.id),
        name: if name.trim().is_empty() { "CardDAV".to_owned() } else { name.trim().to_owned() },
        kind: existing.map(|e| e.kind.clone()).unwrap_or_else(|| "carddav".to_owned()),
        enabled: Some(a.bool_or("enabled", true)),
        base_url: Some(a.opt_str("baseUrl").unwrap_or("").trim().to_owned()),
        collection_path: Some(a.opt_str("collectionPath").unwrap_or("").trim().to_owned()),
        credential_key: existing.map(|e| e.credential_key.clone()),
        default_new_contact: Some(a.bool_or("defaultNewContact", false)),
    })
}

fn id(a: &Args, key: &str) -> Result<i64, String> {
    i64::try_from(a.u64(key)?).map_err(|_| format!("'{key}' is out of range"))
}

/// `Some(result)` if `name` is one of this module's commands.
pub fn run(ctx: &Ctx, name: &str, body: &str) -> Option<Result<Option<String>, String>> {
    if !name.starts_with("contacts.") {
        return None;
    }
    Some(dispatch(ctx, name, body))
}

fn dispatch(ctx: &Ctx, name: &str, body: &str) -> Result<Option<String>, String> {
    let a = Args::parse(body)?;
    match name {
        "contacts.load" => load(ctx).map(|()| None),
        "contacts.repoLinks" => repo_links(ctx, id(&a, "contactId")?).map(|()| None),
        "contacts.repoSave" => {
            let existing = match a.opt_u64("id") {
                Some(i) => frb_contacts_repositories_list()?.into_iter().find(|r| r.id as u64 == i),
                None => None,
            };
            frb_contacts_repository_upsert(repository_upsert(existing.as_ref(), &a)?)?;
            Ok(None)
        }
        "contacts.repoDelete" => frb_contacts_repository_delete(id(&a, "id")?).map(|()| None),
        "contacts.carddav" => carddav(ctx, &a).map(|()| None),
        "contacts.groupSave" => {
            let name = a.str("name")?;
            if name.trim().is_empty() {
                return Err("a group needs a name".to_owned());
            }
            frb_contacts_group_upsert(FrbGroupUpsert { id: None, name: name.trim().to_owned(), color_argb: None })?;
            Ok(None)
        }
        "contacts.groupDelete" => frb_contacts_group_delete(id(&a, "id")?).map(|()| None),
        "contacts.groupAddMember" => {
            frb_contacts_group_add_member(id(&a, "contactId")?, id(&a, "groupId")?).map(|()| None)
        }
        "contacts.groupRemoveMember" => {
            frb_contacts_group_remove_member(id(&a, "contactId")?, id(&a, "groupId")?).map(|()| None)
        }
        "contacts.groupRule" => frb_contacts_set_group_repository_rule(
            id(&a, "groupId")?,
            id(&a, "repositoryId")?,
            a.opt_bool("enable").ok_or("missing boolean argument 'enable'")?,
        )
        .map(|()| None),
        "contacts.applyRules" => {
            let n = frb_contacts_apply_group_repository_rules()?.materialized;
            Ok(Some(event(|w| int(w, "materialized", n))))
        }
        "contacts.allowSharing" => frb_contacts_validate_external_sharing(id(&a, "contactId")?, true).map(|()| None),
        "contacts.setMembership" => frb_contacts_set_repository_membership(
            id(&a, "contactId")?,
            id(&a, "repositoryId")?,
            a.opt_bool("include").ok_or("missing boolean argument 'include'")?,
        )
        .map(|()| None),
        "contacts.delete" => frb_contacts_delete(id(&a, "contactId")?).map(|()| None),
        "contacts.importVcardFile" => {
            let bytes = std::fs::read(a.str("path")?).map_err(|e| format!("could not read the file: {e}"))?;
            let n = frb_contacts_import_vcard_bytes(bytes)?.imported;
            Ok(Some(event(|w| int(w, "imported", i64::from(n)))))
        }
        "contacts.exportVcardFile" => {
            let path = a.str("path")?;
            let text = frb_contacts_export_vcard(Vec::new())?.vcard_text;
            std::fs::write(&path, text).map_err(|e| format!("could not write the file: {e}"))?;
            Ok(Some(event(|w| str(w, "path", &path))))
        }
        _ => Err(format!("unknown command '{name}'")),
    }
}

/// CardDAV pull or push. Credentials are used for this call only. The server
/// is slow and may be wrong, so it runs on its own thread and reports with a
/// `contactsSyncDone` event.
fn carddav(ctx: &Ctx, a: &Args) -> Result<(), String> {
    let action = a.str("action")?;
    let repository = id(a, "id")?;
    let (username, password) = (a.str("username")?, a.str("password")?);
    let request_id = a.str("requestId")?;
    let hub = Arc::clone(&ctx.hub);
    let pull = match action.as_str() {
        "pull" => true,
        "push" => false,
        other => return Err(format!("unknown CardDAV action '{other}'")),
    };
    std::thread::spawn(move || {
        let done = if pull {
            frb_contacts_carddav_pull(repository, username, password).map(|r| {
                (r.ok, r.message, i64::from(r.imported_contacts), i64::from(r.fetched_resources))
            })
        } else {
            frb_contacts_carddav_push(repository, username, password)
                .map(|r| (r.ok, r.message, i64::from(r.pushed), i64::from(r.failed)))
        };
        hub.push(event(|w| {
            str(w, "type", "contactsSyncDone");
            str(w, "requestId", &request_id);
            str(w, "action", if pull { "pull" } else { "push" });
            match &done {
                Ok((ok, message, first, second)) => {
                    flag(w, "ok", *ok);
                    str(w, "message", message);
                    int(w, if pull { "imported" } else { "pushed" }, *first);
                    int(w, if pull { "fetched" } else { "failed" }, *second);
                }
                Err(e) => {
                    flag(w, "ok", false);
                    str(w, "message", e);
                }
            }
        }));
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    fn row(id: i64, kind: &str, key: &str) -> FrbContactRepositoryRow {
        FrbContactRepositoryRow {
            id,
            name: "Old".into(),
            kind: kind.into(),
            enabled: true,
            base_url: "https://dav.example".into(),
            collection_path: "/c/".into(),
            credential_key: key.into(),
            default_new_contact: false,
            ctag: String::new(),
            last_collection_sync_at: None,
            sync_error: "boom \"x\"".into(),
        }
    }

    #[test]
    fn editing_a_repository_keeps_its_kind_and_credential_key() {
        let a = Args::parse(r#"{"id":7,"name":" New ","baseUrl":"https://b","collectionPath":"/p/","enabled":false}"#).unwrap();
        let u = repository_upsert(Some(&row(7, "platform", "vault-key")), &a).unwrap();
        assert_eq!((u.id, u.kind.as_str(), u.name.as_str()), (Some(7), "platform", "New"));
        assert_eq!(u.credential_key.as_deref(), Some("vault-key"));
        assert_eq!(u.enabled, Some(false));

        let fresh = repository_upsert(None, &Args::parse(r#"{"name":""}"#).unwrap()).unwrap();
        assert_eq!((fresh.id, fresh.kind.as_str(), fresh.name.as_str()), (None, "carddav", "CardDAV"));
        assert_eq!(fresh.credential_key, None);
    }

    #[test]
    fn repository_events_escape_and_round_trip() {
        let v = Value::parse(&repository_event(&row(3, "carddav", "k"))).unwrap();
        assert_eq!(v.opt_str("type"), Some("contactsRepository"));
        assert_eq!(v.u64("id").unwrap(), 3);
        assert_eq!(v.opt_str("syncError"), Some("boom \"x\""));
        assert!(v.get("credentialKey").is_none(), "the credential key stays in the back end");
    }
}
