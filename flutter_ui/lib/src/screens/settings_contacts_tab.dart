/*
 * settings_contacts_tab.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Tagliacarte is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_contacts/flutter_contacts.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import '../l10n/app_localizations.dart';
import '../rust/frb_api/frb_contacts.dart';
import '../util/process_log.dart';

PlatformInt64 _p(int i) => PlatformInt64Util.from(i);

/// Settings tab: contacts DB, repositories, groups, vCard import/export.
class SettingsContactsTab extends StatefulWidget {
  const SettingsContactsTab({super.key});

  @override
  State<SettingsContactsTab> createState() => _SettingsContactsTabState();
}

class _SettingsContactsTabState extends State<SettingsContactsTab> {
  bool _loading = true;
  String? _error;
  List<FrbContactRepositoryRow> _repos = <FrbContactRepositoryRow>[];
  List<FrbContactGroupRow> _groups = <FrbContactGroupRow>[];
  List<FrbContactCompactRow> _contacts = <FrbContactCompactRow>[];
  List<FrbGroupRepositoryTargetRow> _groupTargets = <FrbGroupRepositoryTargetRow>[];
  final Map<int, List<FrbContactGroupMemberRow>> _groupMembers =
      <int, List<FrbContactGroupMemberRow>>{};
  /// `null` means links not loaded yet for this contact.
  final Map<int, List<FrbContactRepositoryLinkRow>?> _contactRepoLinks =
      <int, List<FrbContactRepositoryLinkRow>?>{};

  @override
  void initState() {
    super.initState();
    _reload();
  }

  Future<void> _reload() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final List<FrbContactRepositoryRow> repos =
          await frbContactsRepositoriesList();
      final List<FrbContactGroupRow> groups = await frbContactsGroupsList();
      final List<FrbContactCompactRow> contacts =
          await frbContactsListCompact(limit: _p(8000));
      final List<FrbGroupRepositoryTargetRow> targets =
          await frbContactsGroupRepositoryTargetsList();
      if (!mounted) {
        return;
      }
      final Map<int, List<FrbContactGroupMemberRow>> gm =
          <int, List<FrbContactGroupMemberRow>>{};
      for (final FrbContactGroupRow x in groups) {
        final int gid = int.parse(x.id.toString());
        gm[gid] = await frbContactsGroupMembersList(groupId: x.id);
      }
      if (!mounted) {
        return;
      }
      setState(() {
        _repos = repos;
        _groups = groups;
        _contacts = contacts;
        _groupTargets = targets;
        _groupMembers
          ..clear()
          ..addAll(gm);
        _contactRepoLinks.clear();
        _loading = false;
      });
    } catch (e, st) {
      appLogStderr('contacts settings: $e\n$st');
      if (!mounted) {
        return;
      }
      setState(() {
        _error = e.toString();
        _loading = false;
      });
    }
  }

  int? _firstPlatformRepoId() {
    for (final FrbContactRepositoryRow x in _repos) {
      if (x.kind == 'platform') {
        return int.parse(x.id.toString());
      }
    }
    return null;
  }

  bool _groupTargetsRepo(int groupId, int repositoryId) {
    for (final FrbGroupRepositoryTargetRow t in _groupTargets) {
      if (int.parse(t.groupId.toString()) == groupId &&
          int.parse(t.repositoryId.toString()) == repositoryId) {
        return true;
      }
    }
    return false;
  }

  Future<void> _addPlatformRepo(AppLocalizations l10n) async {
    try {
      final FrbContactsRowId row = await frbContactsRepositoryUpsert(
        u: const FrbRepositoryUpsert(
          name: 'Platform',
          kind: 'platform',
          enabled: true,
        ),
      );
      appLogStderr('contacts: added platform repo id=${row.id}');
      await _reload();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.settingsContactsAddPlatformRepo)),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('$e')),
        );
      }
    }
  }

  Future<void> _addCarddavDialog(AppLocalizations l10n) async {
    final TextEditingController name = TextEditingController(text: 'CardDAV');
    final TextEditingController url = TextEditingController();
    final TextEditingController coll = TextEditingController();
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) => AlertDialog(
        title: Text(l10n.settingsContactsAddCarddavRepo),
        content: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              TextField(
                controller: name,
                decoration: InputDecoration(labelText: l10n.settingsContactsRepoName),
              ),
              TextField(
                controller: url,
                decoration: InputDecoration(
                  labelText: l10n.settingsContactsRepoUrl,
                  hintText: 'https://dav.example.com',
                ),
              ),
              TextField(
                controller: coll,
                decoration: InputDecoration(
                  labelText: l10n.settingsContactsCollectionPath,
                  hintText: '/remote.php/dav/addressbooks/users/me/personal/',
                ),
              ),
            ],
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: const Text('Add'),
          ),
        ],
      ),
    );
    if (ok != true || !mounted) {
      return;
    }
    try {
      await frbContactsRepositoryUpsert(
        u: FrbRepositoryUpsert(
          name: name.text.trim().isEmpty ? 'CardDAV' : name.text.trim(),
          kind: 'carddav',
          enabled: true,
          baseUrl: url.text.trim(),
          collectionPath: coll.text.trim(),
        ),
      );
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _editRepositoryDialog(
    AppLocalizations l10n,
    FrbContactRepositoryRow repo,
  ) async {
    final int id = int.parse(repo.id.toString());
    final TextEditingController name = TextEditingController(text: repo.name);
    final TextEditingController base = TextEditingController(text: repo.baseUrl);
    final TextEditingController coll = TextEditingController(text: repo.collectionPath);
    bool enabled = repo.enabled;
    bool defaultNew = repo.defaultNewContact;
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) => StatefulBuilder(
        builder: (BuildContext ctx, void Function(void Function()) setLocal) {
          return AlertDialog(
            title: Text(l10n.settingsContactsEditRepository),
            content: SingleChildScrollView(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  TextField(
                    controller: name,
                    decoration: InputDecoration(labelText: l10n.settingsContactsRepoName),
                  ),
                  TextField(
                    controller: base,
                    decoration: InputDecoration(labelText: l10n.settingsContactsRepoUrl),
                  ),
                  TextField(
                    controller: coll,
                    decoration: InputDecoration(labelText: l10n.settingsContactsCollectionPath),
                  ),
                  SwitchListTile(
                    title: const Text('Enabled'),
                    value: enabled,
                    onChanged: (bool v) => setLocal(() => enabled = v),
                  ),
                  SwitchListTile(
                    title: Text(l10n.settingsContactsDefaultNewContact),
                    value: defaultNew,
                    onChanged: (bool v) => setLocal(() => defaultNew = v),
                  ),
                ],
              ),
            ),
            actions: <Widget>[
              TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('Cancel')),
              FilledButton(onPressed: () => Navigator.pop(ctx, true), child: const Text('Save')),
            ],
          );
        },
      ),
    );
    if (ok != true || !mounted) {
      return;
    }
    try {
      await frbContactsRepositoryUpsert(
        u: FrbRepositoryUpsert(
          id: _p(id),
          name: name.text.trim(),
          kind: repo.kind,
          enabled: enabled,
          baseUrl: base.text.trim(),
          collectionPath: coll.text.trim(),
          defaultNewContact: defaultNew,
        ),
      );
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _deleteRepositoryConfirm(
    AppLocalizations l10n,
    int repositoryId,
    String label,
  ) async {
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) => AlertDialog(
        title: Text(l10n.settingsContactsDeleteRepository),
        content: Text(label),
        actions: <Widget>[
          TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('Cancel')),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: Text(l10n.settingsContactsDeleteRepository),
          ),
        ],
      ),
    );
    if (ok != true || !mounted) {
      return;
    }
    try {
      await frbContactsRepositoryDelete(repositoryId: _p(repositoryId));
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _carddavPushDialog(int repositoryId) async {
    final TextEditingController user = TextEditingController();
    final TextEditingController pass = TextEditingController();
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) {
        final AppLocalizations l10n = AppLocalizations.of(context);
        return AlertDialog(
          title: Text(l10n.settingsContactsCarddavPush),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              TextField(
                controller: user,
                decoration: InputDecoration(labelText: l10n.settingsContactsUsername),
              ),
              TextField(
                controller: pass,
                obscureText: true,
                decoration: InputDecoration(labelText: l10n.settingsContactsPassword),
              ),
            ],
          ),
          actions: <Widget>[
            TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('Cancel')),
            FilledButton(onPressed: () => Navigator.pop(ctx, true), child: const Text('Push')),
          ],
        );
      },
    );
    if (ok != true || !mounted) {
      return;
    }
    try {
      final FrbCarddavPushResult out = await frbContactsCarddavPush(
        repositoryId: _p(repositoryId),
        username: user.text,
        password: pass.text,
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              '${out.message} (${out.pushed} pushed, ${out.failed} failed)',
            ),
          ),
        );
      }
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _carddavPullDialog(int repositoryId) async {
    final TextEditingController user = TextEditingController();
    final TextEditingController pass = TextEditingController();
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) {
        final AppLocalizations l10n = AppLocalizations.of(context);
        return AlertDialog(
          title: Text(l10n.settingsContactsCarddavPull),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              TextField(
                controller: user,
                decoration: InputDecoration(labelText: l10n.settingsContactsUsername),
              ),
              TextField(
                controller: pass,
                obscureText: true,
                decoration: InputDecoration(labelText: l10n.settingsContactsPassword),
              ),
            ],
          ),
          actions: <Widget>[
            TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('Cancel')),
            FilledButton(onPressed: () => Navigator.pop(ctx, true), child: const Text('Pull')),
          ],
        );
      },
    );
    if (ok != true || !mounted) {
      return;
    }
    try {
      final FrbCarddavPullResult out = await frbContactsCarddavPull(
        repositoryId: _p(repositoryId),
        username: user.text,
        password: pass.text,
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              '${out.message} (${out.importedContacts} imported, ${out.fetchedResources} fetched)',
            ),
          ),
        );
      }
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _importVcard(AppLocalizations l10n) async {
    final FilePickerResult? r = await FilePicker.platform.pickFiles(
      type: FileType.custom,
      allowedExtensions: <String>['vcf', 'vcard'],
      withData: true,
    );
    if (r == null || r.files.isEmpty) {
      return;
    }
    final PlatformFile f = r.files.first;
    final List<int>? b = f.bytes;
    if (b == null) {
      return;
    }
    try {
      final FrbImportVcardResult out = await frbContactsImportVcardBytes(bytes: b);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Imported ${out.imported} contacts')),
        );
      }
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _exportVcard() async {
    try {
      final FrbExportedVcard export =
          await frbContactsExportVcard(contactIds: Int64List(0));
      final String? path = await FilePicker.platform.saveFile(
        dialogTitle: 'Export contacts',
        fileName: 'contacts.vcf',
        type: FileType.custom,
        allowedExtensions: <String>['vcf'],
      );
      if (path == null) {
        return;
      }
      final File file = File(path);
      await file.writeAsString(export.vcardText);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Saved $path')),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _mergePlatform(AppLocalizations l10n) async {
    if (!Platform.isAndroid && !Platform.isIOS) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('System contacts import is for Android and iOS.'),
          ),
        );
      }
      return;
    }
    final int? platformRid = _firstPlatformRepoId();
    bool linkToPlatform = platformRid != null;
    final bool? confirm = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) => StatefulBuilder(
        builder: (BuildContext ctx, void Function(void Function()) setLocal) {
          return AlertDialog(
            title: Text(l10n.settingsContactsMergePlatform),
            content: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                if (platformRid != null)
                  CheckboxListTile(
                    title: Text(l10n.settingsContactsLinkPlatformMerge),
                    value: linkToPlatform,
                    onChanged: (bool? v) => setLocal(() => linkToPlatform = v ?? false),
                  )
                else
                  Text(
                    'Add a platform repository first to link imports.',
                    style: Theme.of(ctx).textTheme.bodySmall,
                  ),
              ],
            ),
            actions: <Widget>[
              TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('Cancel')),
              FilledButton(onPressed: () => Navigator.pop(ctx, true), child: const Text('Continue')),
            ],
          );
        },
      ),
    );
    if (confirm != true || !mounted) {
      return;
    }
    try {
      if (!await FlutterContacts.requestPermission()) {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text(l10n.settingsContactsMergePlatform)),
          );
        }
        return;
      }
      final List<Contact> contacts = await FlutterContacts.getContacts(
        withProperties: true,
      );
      final List<FrbPlatformContactItem> items = <FrbPlatformContactItem>[];
      for (final Contact c in contacts) {
        final List<String> emails = <String>[];
        for (final Email e in c.emails) {
          final String a = e.address.trim();
          if (a.isNotEmpty) {
            emails.add(a);
          }
        }
        if (emails.isEmpty) {
          continue;
        }
        items.add(FrbPlatformContactItem(
          displayName: c.displayName,
          emails: emails,
        ));
      }
      final FrbMergePlatformResult res = await frbContactsMergePlatform(
        req: FrbMergePlatformContacts(
          items: items,
          repositoryId: linkToPlatform && platformRid != null
              ? _p(platformRid)
              : null,
        ),
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Imported ${res.imported} contacts')),
        );
      }
      await _reload();
    } catch (e, st) {
      appLogStderr('merge platform contacts: $e\n$st');
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _newGroupDialog(AppLocalizations l10n) async {
    final TextEditingController t = TextEditingController();
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) => AlertDialog(
        title: Text(l10n.settingsContactsNewGroup),
        content: TextField(
          controller: t,
          decoration: InputDecoration(labelText: l10n.settingsContactsGroups),
        ),
        actions: <Widget>[
          TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('Cancel')),
          FilledButton(onPressed: () => Navigator.pop(ctx, true), child: const Text('Create')),
        ],
      ),
    );
    if (ok != true || !mounted) {
      return;
    }
    final String name = t.text.trim();
    if (name.isEmpty) {
      return;
    }
    try {
      await frbContactsGroupUpsert(u: FrbGroupUpsert(name: name));
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _deleteGroup(int groupId, AppLocalizations l10n) async {
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) => AlertDialog(
        title: Text(l10n.settingsContactsGroups),
        content: const Text('Delete this group? Membership is removed; contacts are kept.'),
        actions: <Widget>[
          TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('Cancel')),
          FilledButton(onPressed: () => Navigator.pop(ctx, true), child: const Text('Delete')),
        ],
      ),
    );
    if (ok != true || !mounted) {
      return;
    }
    try {
      await frbContactsGroupDelete(groupId: _p(groupId));
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _setGroupRepoRule(int groupId, int repositoryId, bool enable) async {
    try {
      await frbContactsSetGroupRepositoryRule(
        groupId: _p(groupId),
        repositoryId: _p(repositoryId),
        enable: enable,
      );
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _applyGroupRules(AppLocalizations l10n) async {
    try {
      final FrbContactsApplyGroupRulesResult r =
          await frbContactsApplyGroupRepositoryRules();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Applied rules (${r.materialized} memberships)'),
          ),
        );
      }
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _loadRepoLinks(int contactId) async {
    if (_contactRepoLinks.containsKey(contactId)) {
      return;
    }
    setState(() {
      _contactRepoLinks[contactId] = null;
    });
    try {
      final List<FrbContactRepositoryLinkRow> links =
          await frbContactsRepositoryLinksForContact(contactId: _p(contactId));
      if (!mounted) {
        return;
      }
      setState(() {
        _contactRepoLinks[contactId] = links;
      });
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
        setState(() {
          _contactRepoLinks.remove(contactId);
        });
      }
    }
  }

  Future<void> _toggleRepoMembership(
    int contactId,
    int repositoryId,
    bool include,
  ) async {
    try {
      await frbContactsSetRepositoryMembership(
        contactId: _p(contactId),
        repositoryId: _p(repositoryId),
        include: include,
      );
      _contactRepoLinks.remove(contactId);
      await _loadRepoLinks(contactId);
      if (mounted) {
        await _reload();
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _validateExternal(int contactId, bool ok) async {
    try {
      await frbContactsValidateExternalSharing(contactId: _p(contactId), ok: ok);
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _addContactToGroup(int groupId) async {
    final List<int> memberIds = (_groupMembers[groupId] ?? <FrbContactGroupMemberRow>[])
        .map((FrbContactGroupMemberRow x) => int.parse(x.id.toString()))
        .toList();
    final List<FrbContactCompactRow> choices = _contacts
        .where(
          (FrbContactCompactRow c) =>
              !memberIds.contains(int.parse(c.id.toString())),
        )
        .toList();
    if (choices.isEmpty) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('No contacts to add')),
        );
      }
      return;
    }
    int? pick = int.parse(choices.first.id.toString());
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) => StatefulBuilder(
        builder: (BuildContext ctx, void Function(void Function()) setLocal) {
          return AlertDialog(
            title: const Text('Add to group'),
            content: DropdownButton<int>(
              value: pick,
              isExpanded: true,
              items: choices
                  .map(
                    (FrbContactCompactRow c) => DropdownMenuItem<int>(
                      value: int.parse(c.id.toString()),
                      child: Text(
                        '${c.displayName} ${c.primaryEmail ?? ''}'.trim(),
                      ),
                    ),
                  )
                  .toList(),
              onChanged: (int? v) => setLocal(() => pick = v),
            ),
            actions: <Widget>[
              TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('Cancel')),
              FilledButton(onPressed: () => Navigator.pop(ctx, true), child: const Text('Add')),
            ],
          );
        },
      ),
    );
    if (ok != true || pick == null || !mounted) {
      return;
    }
    try {
      await frbContactsGroupAddMember(contactId: _p(pick!), groupId: _p(groupId));
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    if (_loading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_error != null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: SelectableText(_error!),
        ),
      );
    }
    return ListView(
      padding: const EdgeInsets.all(16),
      children: <Widget>[
        Text(
          l10n.settingsContactsLearnedNote,
          style: Theme.of(context).textTheme.bodySmall,
        ),
        const SizedBox(height: 16),
        Text(l10n.settingsContactsRepositories, style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        ..._repos.map((FrbContactRepositoryRow r) {
          final int id = int.parse(r.id.toString());
          final String kind = r.kind;
          return ListTile(
            title: Text(r.name),
            subtitle: Text(
              <String>[
                kind,
                if (!r.enabled) 'off',
                if (r.syncError.isNotEmpty) r.syncError,
              ].where((String s) => s.isNotEmpty).join(' · '),
            ),
            trailing: PopupMenuButton<String>(
              onSelected: (String v) async {
                if (v == 'edit') {
                  await _editRepositoryDialog(l10n, r);
                } else if (v == 'del') {
                  await _deleteRepositoryConfirm(
                    l10n,
                    id,
                    r.name,
                  );
                } else if (v == 'sync') {
                  try {
                    final FrbContactsSyncRepositoryResult s =
                        await frbContactsSyncRepository(repositoryId: _p(id));
                    if (!context.mounted) {
                      return;
                    }
                    ScaffoldMessenger.of(context)
                        .showSnackBar(SnackBar(content: Text(s.message)));
                    await _reload();
                  } catch (e) {
                    if (!context.mounted) {
                      return;
                    }
                    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
                  }
                } else if (v == 'pull' && kind == 'carddav') {
                  await _carddavPullDialog(id);
                } else if (v == 'push' && kind == 'carddav') {
                  await _carddavPushDialog(id);
                }
              },
              itemBuilder: (BuildContext ctx) => <PopupMenuEntry<String>>[
                PopupMenuItem<String>(value: 'edit', child: Text(l10n.settingsContactsEditRepository)),
                PopupMenuItem<String>(value: 'sync', child: Text(l10n.settingsContactsRepositories)),
                if (kind == 'carddav') ...<PopupMenuEntry<String>>[
                  PopupMenuItem<String>(value: 'pull', child: Text(l10n.settingsContactsCarddavPull)),
                  PopupMenuItem<String>(value: 'push', child: Text(l10n.settingsContactsCarddavPush)),
                ],
                PopupMenuItem<String>(
                  value: 'del',
                  child: Text(l10n.settingsContactsDeleteRepository),
                ),
              ],
            ),
          );
        }),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            OutlinedButton(
              onPressed: () => _addPlatformRepo(l10n),
              child: Text(l10n.settingsContactsAddPlatformRepo),
            ),
            OutlinedButton(
              onPressed: () => _addCarddavDialog(l10n),
              child: Text(l10n.settingsContactsAddCarddavRepo),
            ),
          ],
        ),
        const Divider(height: 32),
        Text(l10n.settingsContactsGroups, style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 4),
        Text(l10n.settingsContactsGroupShareHint, style: Theme.of(context).textTheme.bodySmall),
        const SizedBox(height: 8),
        ..._groups.map((FrbContactGroupRow g) {
          final int gid = int.parse(g.id.toString());
          final List<FrbContactGroupMemberRow> members =
              _groupMembers[gid] ?? <FrbContactGroupMemberRow>[];
          return Card(
            margin: const EdgeInsets.only(bottom: 8),
            child: ExpansionTile(
              title: Text(g.name),
              subtitle: Text('id $gid · ${members.length} members'),
              children: <Widget>[
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: Wrap(
                      spacing: 8,
                      children: <Widget>[
                        TextButton(
                          onPressed: () => _addContactToGroup(gid),
                          child: const Text('Add member'),
                        ),
                        TextButton(
                          onPressed: () => _deleteGroup(gid, l10n),
                          child: Text(l10n.settingsContactsDeleteRepository),
                        ),
                      ],
                    ),
                  ),
                ),
                for (final FrbContactRepositoryRow repo in _repos)
                  Builder(
                    builder: (BuildContext ctx) {
                      final int rid = int.parse(repo.id.toString());
                      return SwitchListTile(
                        title: Text(repo.name),
                        subtitle: Text(repo.kind),
                        value: _groupTargetsRepo(gid, rid),
                        onChanged: (bool v) => _setGroupRepoRule(gid, rid, v),
                      );
                    },
                  ),
                Padding(
                  padding: const EdgeInsets.all(16),
                  child: FilledButton(
                    onPressed: () => _applyGroupRules(l10n),
                    child: Text(l10n.settingsContactsApplyRules),
                  ),
                ),
                const Divider(),
                ...members.map((FrbContactGroupMemberRow mem) {
                  final int cid = int.parse(mem.id.toString());
                  return ListTile(
                    dense: true,
                    title: Text(mem.displayName),
                    subtitle: Text(mem.primaryEmail ?? ''),
                    trailing: IconButton(
                      icon: const Icon(Icons.remove_circle_outline),
                      onPressed: () async {
                        try {
                          await frbContactsGroupRemoveMember(
                            contactId: _p(cid),
                            groupId: _p(gid),
                          );
                          await _reload();
                        } catch (e) {
                          if (!mounted) {
                            return;
                          }
                          ScaffoldMessenger.of(this.context).showSnackBar(
                            SnackBar(content: Text('$e')),
                          );
                        }
                      },
                    ),
                  );
                }),
              ],
            ),
          );
        }),
        OutlinedButton(
          onPressed: () => _newGroupDialog(l10n),
          child: Text(l10n.settingsContactsNewGroup),
        ),
        const Divider(height: 32),
        Text(l10n.settingsContactsLocalContacts, style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        ..._contacts.map((FrbContactCompactRow c) {
          final int cid = int.parse(c.id.toString());
          final bool shareOk = c.externallyShareOk;
          final String origin = c.importOrigin;
          final bool needsGate = !shareOk && origin == 'learned_from_mail';
          return Card(
            margin: const EdgeInsets.only(bottom: 6),
            child: ExpansionTile(
              onExpansionChanged: (bool ex) {
                if (ex) {
                  _loadRepoLinks(cid);
                }
              },
              title: Text(c.displayName),
              subtitle: Text(
                '${c.primaryEmail ?? ''} · $origin'.trim(),
              ),
              children: <Widget>[
                if (needsGate)
                  ListTile(
                    leading: const Icon(Icons.lock_outline),
                    title: Text(l10n.settingsContactsAllowExternalSync),
                    trailing: FilledButton(
                      onPressed: () => _validateExternal(cid, true),
                      child: Text(l10n.settingsContactsAllowExternalSync),
                    ),
                  ),
                if (_contactRepoLinks[cid] == null)
                  const ListTile(
                    dense: true,
                    leading: SizedBox(
                      width: 24,
                      height: 24,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                    title: Text('Loading sync targets…'),
                  )
                else
                  ...(_contactRepoLinks[cid]!).map((FrbContactRepositoryLinkRow lk) {
                    final int rid = int.parse(lk.repositoryId.toString());
                    final bool linked = lk.linked;
                    final bool canToggle = shareOk && !needsGate;
                    return SwitchListTile(
                      title: Text(lk.name),
                      subtitle: Text(lk.kind),
                      value: linked,
                      onChanged: canToggle
                          ? (bool v) => _toggleRepoMembership(cid, rid, v)
                          : null,
                    );
                  }),
              ],
            ),
          );
        }),
        const Divider(height: 32),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            FilledButton(
              onPressed: () => _importVcard(l10n),
              child: Text(l10n.settingsContactsImportVcard),
            ),
            OutlinedButton(
              onPressed: _exportVcard,
              child: Text(l10n.settingsContactsExportVcard),
            ),
            if (Platform.isAndroid || Platform.isIOS)
              OutlinedButton(
                onPressed: () => _mergePlatform(l10n),
                child: Text(l10n.settingsContactsMergePlatform),
              ),
          ],
        ),
      ],
    );
  }
}
