/*
 * contacts_screen.dart
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This file is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this file.  If not, see <http://www.gnu.org/licenses/>.
 */

import 'dart:async';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_contacts/flutter_contacts.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

import '../l10n/app_localizations.dart';
import '../layout/mail_layout.dart';
import '../rust/frb_api/frb_contacts.dart';
import '../util/process_log.dart';

PlatformInt64 _pi(int i) => PlatformInt64Util.from(i);

int _p64(PlatformInt64 x) => int.parse(x.toString());

/// Fullscreen contact editor (compact layout / pushed route).
class ContactDetailScreen extends StatefulWidget {
  const ContactDetailScreen({
    super.key,
    required this.contactId,
  });

  /// When null, creates a new contact on save.
  final int? contactId;

  @override
  State<ContactDetailScreen> createState() => _ContactDetailScreenState();
}

class _ContactDetailScreenState extends State<ContactDetailScreen> {
  bool _loading = true;
  List<FrbContactGroupRow> _groups = <FrbContactGroupRow>[];
  final Map<int, List<FrbContactGroupMemberRow>> _groupMembers =
      <int, List<FrbContactGroupMemberRow>>{};

  @override
  void initState() {
    super.initState();
    unawaited(_loadGroups());
  }

  Future<void> _loadGroups() async {
    try {
      final List<FrbContactGroupRow> groups = await frbContactsGroupsList();
      final Map<int, List<FrbContactGroupMemberRow>> gm =
          <int, List<FrbContactGroupMemberRow>>{};
      for (final FrbContactGroupRow x in groups) {
        final int gid = _p64(x.id);
        gm[gid] = await frbContactsGroupMembersList(groupId: x.id);
      }
      if (!mounted) {
        return;
      }
      setState(() {
        _groups = groups;
        _groupMembers
          ..clear()
          ..addAll(gm);
        _loading = false;
      });
    } catch (e, st) {
      appLogStderr('contact detail groups: $e\n$st');
      if (mounted) {
        setState(() => _loading = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.contactsDetailTitle),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : ContactDetailEditor(
              contactId: widget.contactId,
              embedded: false,
              groups: _groups,
              groupMembers: _groupMembers,
              onSaved: () => Navigator.of(context).pop(true),
              onDeleted: () => Navigator.of(context).pop(true),
            ),
    );
  }
}

/// Mail contacts manager: groups, list, sync/import/delete, detail.
class ContactsScreen extends StatefulWidget {
  const ContactsScreen({super.key});

  @override
  State<ContactsScreen> createState() => _ContactsScreenState();
}

class _ContactsScreenState extends State<ContactsScreen> {
  bool _loading = true;
  String? _error;
  List<FrbContactRepositoryRow> _repos = <FrbContactRepositoryRow>[];
  List<FrbContactGroupRow> _groups = <FrbContactGroupRow>[];
  List<FrbContactCompactRow> _contacts = <FrbContactCompactRow>[];
  List<FrbGroupRepositoryTargetRow> _groupTargets =
      <FrbGroupRepositoryTargetRow>[];
  final Map<int, List<FrbContactGroupMemberRow>> _groupMembers =
      <int, List<FrbContactGroupMemberRow>>{};

  /// `null` = show all contacts; otherwise filter to group id.
  int? _filterGroupId;
  int? _detailContactId;
  bool _creatingNew = false;
  bool _selectionMode = false;
  final Set<int> _selectedContactIds = <int>{};

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
          await frbContactsListCompact(limit: _pi(8000));
      final List<FrbGroupRepositoryTargetRow> targets =
          await frbContactsGroupRepositoryTargetsList();
      if (!mounted) {
        return;
      }
      final Map<int, List<FrbContactGroupMemberRow>> gm =
          <int, List<FrbContactGroupMemberRow>>{};
      for (final FrbContactGroupRow x in groups) {
        final int gid = _p64(x.id);
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
        _loading = false;
      });
    } catch (e, st) {
      appLogStderr('contacts screen: $e\n$st');
      if (!mounted) {
        return;
      }
      setState(() {
        _error = e.toString();
        _loading = false;
      });
    }
  }

  @override
  void initState() {
    super.initState();
    unawaited(_reload());
  }

  List<FrbContactCompactRow> get _filteredContacts {
    if (_filterGroupId == null) {
      return _contacts;
    }
    final List<FrbContactGroupMemberRow>? mem = _groupMembers[_filterGroupId];
    if (mem == null) {
      return <FrbContactCompactRow>[];
    }
    final Set<int> ids = <int>{for (final FrbContactGroupMemberRow m in mem) _p64(m.id)};
    return _contacts.where((FrbContactCompactRow c) => ids.contains(_p64(c.id))).toList();
  }

  bool _groupTargetsRepo(int groupId, int repositoryId) {
    for (final FrbGroupRepositoryTargetRow t in _groupTargets) {
      if (_p64(t.groupId) == groupId && _p64(t.repositoryId) == repositoryId) {
        return true;
      }
    }
    return false;
  }

  List<FrbContactRepositoryRow> _reposForSelectedGroup() {
    if (_filterGroupId == null) {
      return <FrbContactRepositoryRow>[];
    }
    final List<FrbContactRepositoryRow> out = <FrbContactRepositoryRow>[];
    for (final FrbContactRepositoryRow r in _repos) {
      if (_groupTargetsRepo(_filterGroupId!, _p64(r.id))) {
        out.add(r);
      }
    }
    return out;
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
          SnackBar(content: Text(l10n.contactsImportDone(out.imported))),
        );
      }
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _carddavPull(int repositoryId) async {
    final TextEditingController user = TextEditingController();
    final TextEditingController pass = TextEditingController();
    final AppLocalizations l10n = AppLocalizations.of(context);
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) => AlertDialog(
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
          TextButton(onPressed: () => Navigator.pop(ctx, false), child: Text(l10n.cancel)),
          FilledButton(onPressed: () => Navigator.pop(ctx, true), child: Text(l10n.dialogOk)),
        ],
      ),
    );
    if (ok != true || !mounted) {
      return;
    }
    try {
      final FrbCarddavPullResult out = await frbContactsCarddavPull(
        repositoryId: _pi(repositoryId),
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

  Future<void> _carddavPush(int repositoryId) async {
    final TextEditingController user = TextEditingController();
    final TextEditingController pass = TextEditingController();
    final AppLocalizations l10n = AppLocalizations.of(context);
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) => AlertDialog(
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
          TextButton(onPressed: () => Navigator.pop(ctx, false), child: Text(l10n.cancel)),
          FilledButton(onPressed: () => Navigator.pop(ctx, true), child: Text(l10n.dialogOk)),
        ],
      ),
    );
    if (ok != true || !mounted) {
      return;
    }
    try {
      final FrbCarddavPushResult out = await frbContactsCarddavPush(
        repositoryId: _pi(repositoryId),
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

  int? _firstPlatformRepoId() {
    for (final FrbContactRepositoryRow x in _repos) {
      if (x.kind == 'platform') {
        return _p64(x.id);
      }
    }
    return null;
  }

  Future<void> _mergePlatform(AppLocalizations l10n) async {
    if (!Platform.isAndroid && !Platform.isIOS) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.settingsContactsMergePlatform)),
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
                    l10n.settingsContactsAddPlatformRepo,
                    style: Theme.of(ctx).textTheme.bodySmall,
                  ),
              ],
            ),
            actions: <Widget>[
              TextButton(onPressed: () => Navigator.pop(ctx, false), child: Text(l10n.cancel)),
              FilledButton(onPressed: () => Navigator.pop(ctx, true), child: Text(l10n.dialogOk)),
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
        return;
      }
      final List<Contact> contacts = await FlutterContacts.getContacts(withProperties: true);
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
          repositoryId: linkToPlatform && platformRid != null ? _pi(platformRid) : null,
        ),
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.contactsImportDone(res.imported))),
        );
      }
      await _reload();
    } catch (e, st) {
      appLogStderr('merge platform: $e\n$st');
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  void _showSyncMenu(AppLocalizations l10n) {
    if (_filterGroupId == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.contactsSyncNeedGroup)),
      );
      return;
    }
    final List<FrbContactRepositoryRow> scoped = _reposForSelectedGroup();
    showModalBottomSheet<void>(
      context: context,
      builder: (BuildContext ctx) {
        final List<Widget> items = <Widget>[
          for (final FrbContactRepositoryRow r in scoped)
            if (r.kind == 'carddav') ...<Widget>[
              ListTile(
                title: Text(l10n.contactsSyncMenuPull(r.name)),
                onTap: () {
                  Navigator.pop(ctx);
                  unawaited(_carddavPull(_p64(r.id)));
                },
              ),
              ListTile(
                title: Text(l10n.contactsSyncMenuPush(r.name)),
                onTap: () {
                  Navigator.pop(ctx);
                  unawaited(_carddavPush(_p64(r.id)));
                },
              ),
            ],
          if (Platform.isAndroid || Platform.isIOS)
            ListTile(
              leading: const Icon(Icons.phone_android),
              title: Text(l10n.contactsMergePlatform),
              onTap: () {
                Navigator.pop(ctx);
                unawaited(_mergePlatform(l10n));
              },
            ),
        ];
        if (items.isEmpty) {
          return SafeArea(
            child: Padding(
              padding: const EdgeInsets.all(24),
              child: Text(l10n.contactsSyncNeedGroup),
            ),
          );
        }
        return SafeArea(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: items,
          ),
        );
      },
    );
  }

  Future<void> _deleteSelectedContacts(AppLocalizations l10n) async {
    if (_selectedContactIds.isEmpty) {
      return;
    }
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) => AlertDialog(
        title: Text(l10n.contactsDeleteSelectedTitle),
        content: Text(l10n.contactsDeleteSelectedBody(_selectedContactIds.length)),
        actions: <Widget>[
          TextButton(onPressed: () => Navigator.pop(ctx, false), child: Text(l10n.cancel)),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: Text(l10n.delete),
          ),
        ],
      ),
    );
    if (ok != true || !mounted) {
      return;
    }
    try {
      for (final int id in _selectedContactIds) {
        await frbContactsDelete(contactId: _pi(id));
      }
      setState(() {
        _selectedContactIds.clear();
        _selectionMode = false;
        _detailContactId = null;
        _creatingNew = false;
      });
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _deleteCurrentGroup(AppLocalizations l10n) async {
    if (_filterGroupId == null) {
      return;
    }
    String name = '';
    for (final FrbContactGroupRow g in _groups) {
      if (_p64(g.id) == _filterGroupId) {
        name = g.name;
        break;
      }
    }
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) => AlertDialog(
        title: Text(l10n.contactsDeleteGroupTitle),
        content: Text(l10n.contactsDeleteGroupBody(name)),
        actions: <Widget>[
          TextButton(onPressed: () => Navigator.pop(ctx, false), child: Text(l10n.cancel)),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: Text(l10n.delete),
          ),
        ],
      ),
    );
    if (ok != true || !mounted) {
      return;
    }
    try {
      await frbContactsGroupDelete(groupId: _pi(_filterGroupId!));
      setState(() {
        _filterGroupId = null;
        _detailContactId = null;
      });
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _pickRepositoryToDelete(AppLocalizations l10n) async {
    final FrbContactRepositoryRow? picked = await showDialog<FrbContactRepositoryRow>(
      context: context,
      builder: (BuildContext ctx) => SimpleDialog(
        title: Text(l10n.contactsDeleteRepositoryTitle),
        children: <Widget>[
          for (final FrbContactRepositoryRow r in _repos)
            SimpleDialogOption(
              onPressed: () => Navigator.pop(ctx, r),
              child: Text('${r.name} (${r.kind})'),
            ),
        ],
      ),
    );
    if (picked == null || !mounted) {
      return;
    }
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) => AlertDialog(
        title: Text(l10n.contactsDeleteRepositoryTitle),
        content: Text(l10n.contactsDeleteRepositoryBody(picked.name)),
        actions: <Widget>[
          TextButton(onPressed: () => Navigator.pop(ctx, false), child: Text(l10n.cancel)),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: Text(l10n.delete),
          ),
        ],
      ),
    );
    if (ok != true || !mounted) {
      return;
    }
    try {
      await frbContactsRepositoryDelete(repositoryId: picked.id);
      await _reload();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  void _showDeleteMenu(AppLocalizations l10n) {
    showModalBottomSheet<void>(
      context: context,
      builder: (BuildContext ctx) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            if (_selectionMode && _selectedContactIds.isNotEmpty)
              ListTile(
                leading: const Icon(Icons.person_remove_outlined),
                title: Text(l10n.contactsDeleteMenuContacts),
                onTap: () {
                  Navigator.pop(ctx);
                  unawaited(_deleteSelectedContacts(l10n));
                },
              ),
            if (_filterGroupId != null)
              ListTile(
                leading: const Icon(Icons.folder_delete_outlined),
                title: Text(l10n.contactsDeleteMenuGroup),
                onTap: () {
                  Navigator.pop(ctx);
                  unawaited(_deleteCurrentGroup(l10n));
                },
              ),
            ListTile(
              leading: const Icon(Icons.storage_outlined),
              title: Text(l10n.contactsDeleteMenuRepository),
              onTap: () {
                Navigator.pop(ctx);
                unawaited(_pickRepositoryToDelete(l10n));
              },
            ),
          ],
        ),
      ),
    );
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
          TextButton(onPressed: () => Navigator.pop(ctx, false), child: Text(l10n.cancel)),
          FilledButton(onPressed: () => Navigator.pop(ctx, true), child: Text(l10n.create)),
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

  Widget _groupsListPane(AppLocalizations l10n, {bool drawer = false}) {
    return Material(
      child: ListView(
        padding: const EdgeInsets.symmetric(vertical: 8),
        children: <Widget>[
          ListTile(
            title: Text(l10n.contactsAllGroups),
            selected: _filterGroupId == null,
            onTap: () {
              setState(() => _filterGroupId = null);
              if (drawer) {
                Navigator.of(context).pop();
              }
            },
          ),
          const Divider(height: 1),
          for (final FrbContactGroupRow g in _groups)
            ListTile(
              title: Text(g.name),
              selected: _filterGroupId == _p64(g.id),
              subtitle: _groupHasSyncTarget(_p64(g.id))
                  ? Text(
                      l10n.contactsToolbarSync,
                      style: Theme.of(context).textTheme.bodySmall,
                    )
                  : null,
              onTap: () {
                setState(() => _filterGroupId = _p64(g.id));
                if (drawer) {
                  Navigator.of(context).pop();
                }
              },
            ),
        ],
      ),
    );
  }

  bool _groupHasSyncTarget(int groupId) {
    for (final FrbGroupRepositoryTargetRow t in _groupTargets) {
      if (_p64(t.groupId) == groupId) {
        return true;
      }
    }
    return false;
  }

  Widget _buildToolbar(AppLocalizations l10n) {
    return Material(
      elevation: 1,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
        child: Row(
          children: <Widget>[
            IconButton(
              tooltip: l10n.contactsToolbarImportTooltip,
              icon: const Icon(Icons.upload_file_outlined),
              onPressed: () => unawaited(_importVcard(l10n)),
            ),
            IconButton(
              tooltip: l10n.contactsToolbarSyncTooltip,
              icon: const Icon(Icons.sync),
              onPressed: () => _showSyncMenu(l10n),
            ),
            IconButton(
              tooltip: l10n.contactsToolbarDeleteTooltip,
              icon: const Icon(Icons.delete_outline),
              onPressed: () => _showDeleteMenu(l10n),
            ),
            const Spacer(),
            IconButton(
              tooltip: l10n.contactsToolbarNewGroupTooltip,
              icon: const Icon(Icons.create_new_folder_outlined),
              onPressed: () => unawaited(_newGroupDialog(l10n)),
            ),
            TextButton(
              onPressed: () {
                setState(() {
                  _selectionMode = !_selectionMode;
                  if (!_selectionMode) {
                    _selectedContactIds.clear();
                  }
                });
              },
              child: Text(_selectionMode ? l10n.contactsSelectDone : l10n.contactsSelect),
            ),
          ],
        ),
      ),
    );
  }

  Widget _contactRowTile(
    FrbContactCompactRow c,
    AppLocalizations l10n, {
    required bool useWideDetail,
  }) {
    final int cid = _p64(c.id);
    final bool selected = _selectedContactIds.contains(cid);
    final bool detailSel = _detailContactId == cid && !_creatingNew;
    return ListTile(
      leading: CircleAvatar(
        child: Text(
          _initials(c.displayName),
          style: const TextStyle(fontSize: 14),
        ),
      ),
      title: Text(
        c.displayName.isEmpty ? (c.primaryEmail ?? '') : c.displayName,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: c.primaryEmail != null
          ? Text(c.primaryEmail!, maxLines: 1, overflow: TextOverflow.ellipsis)
          : null,
      selected: detailSel,
      trailing: _selectionMode
          ? Checkbox(
              value: selected,
              onChanged: (bool? v) {
                setState(() {
                  if (v == true) {
                    _selectedContactIds.add(cid);
                  } else {
                    _selectedContactIds.remove(cid);
                  }
                });
              },
            )
          : null,
      onTap: () {
        if (_selectionMode) {
          setState(() {
            if (selected) {
              _selectedContactIds.remove(cid);
            } else {
              _selectedContactIds.add(cid);
            }
          });
        } else if (useWideDetail) {
          setState(() {
            _detailContactId = cid;
            _creatingNew = false;
          });
        } else {
          unawaited(_openDetailCompact(cid, false));
        }
      },
      onLongPress: () {
        setState(() {
          _selectionMode = true;
          _selectedContactIds.add(cid);
        });
      },
    );
  }

  String _initials(String name) {
    final String t = name.trim();
    if (t.isEmpty) {
      return '?';
    }
    final List<String> parts = t.split(RegExp(r'\s+'));
    if (parts.length >= 2) {
      final String a = parts.first.isNotEmpty ? parts.first[0] : '';
      final String b = parts.last.isNotEmpty ? parts.last[0] : '';
      return '$a$b'.toUpperCase();
    }
    if (t.length >= 2) {
      return t.substring(0, 2).toUpperCase();
    }
    return t.toUpperCase();
  }

  Future<void> _openDetailCompact(int? contactId, bool creating) async {
    final Object? res = await Navigator.of(context).push<Object?>(
      MaterialPageRoute<Object?>(
        builder: (_) => ContactDetailScreen(contactId: creating ? null : contactId),
      ),
    );
    if (res == true && mounted) {
      await _reload();
    }
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = AppLocalizations.of(context);
    if (_loading) {
      return Scaffold(
        appBar: AppBar(title: Text(l10n.contactsScreenTitle)),
        body: const Center(child: CircularProgressIndicator()),
      );
    }
    if (_error != null) {
      return Scaffold(
        appBar: AppBar(title: Text(l10n.contactsScreenTitle)),
        body: Center(child: Text(l10n.contactsLoadError(_error!))),
      );
    }

    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final bool compact = mailLayoutIsCompact(
          constraints.maxWidth,
          constraints.maxHeight,
        );
        final List<FrbContactCompactRow> rows = _filteredContacts;

        if (compact) {
          return Scaffold(
            appBar: AppBar(
              title: Text(l10n.contactsScreenTitle),
              actions: <Widget>[
                IconButton(
                  tooltip: l10n.contactsNewContact,
                  icon: const Icon(Icons.person_add_outlined),
                  onPressed: () => unawaited(_openDetailCompact(null, true)),
                ),
              ],
            ),
            drawer: Drawer(
              child: _groupsListPane(l10n, drawer: true),
            ),
            body: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                _buildToolbar(l10n),
                Expanded(
                  child: rows.isEmpty
                      ? Center(child: Text(l10n.contactsEmptyList))
                      : ListView.builder(
                          itemCount: rows.length,
                          itemBuilder: (BuildContext context, int i) =>
                              _contactRowTile(rows[i], l10n, useWideDetail: false),
                        ),
                ),
              ],
            ),
          );
        }

        return Scaffold(
          appBar: AppBar(
            title: Text(l10n.contactsScreenTitle),
            actions: <Widget>[
              IconButton(
                tooltip: l10n.contactsNewContact,
                icon: const Icon(Icons.person_add_outlined),
                onPressed: () {
                  setState(() {
                    _creatingNew = true;
                    _detailContactId = null;
                  });
                },
              ),
            ],
          ),
          body: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              SizedBox(
                width: 220,
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    border: Border(
                      right: BorderSide(color: Theme.of(context).dividerColor),
                    ),
                  ),
                  child: _groupsListPane(l10n),
                ),
              ),
              Expanded(
                flex: 5,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    _buildToolbar(l10n),
                    Expanded(
                      child: rows.isEmpty
                          ? Center(child: Text(l10n.contactsEmptyList))
                          : ListView.builder(
                              itemCount: rows.length,
                              itemBuilder: (BuildContext context, int i) =>
                                  _contactRowTile(rows[i], l10n, useWideDetail: true),
                            ),
                    ),
                  ],
                ),
              ),
              Expanded(
                flex: 6,
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    border: Border(
                      left: BorderSide(color: Theme.of(context).dividerColor),
                    ),
                  ),
                  child: _creatingNew || _detailContactId != null
                      ? ContactDetailEditor(
                          key: ValueKey<Object?>(
                            _creatingNew ? 'new' : _detailContactId,
                          ),
                          contactId: _creatingNew ? null : _detailContactId,
                          embedded: true,
                          groups: _groups,
                          groupMembers: _groupMembers,
                          onCreated: (int id) {
                            setState(() {
                              _creatingNew = false;
                              _detailContactId = id;
                            });
                          },
                          onSaved: () async {
                            await _reload();
                          },
                          onDeleted: () async {
                            await _reload();
                            setState(() {
                              _detailContactId = null;
                              _creatingNew = false;
                            });
                          },
                        )
                      : Center(child: Text(l10n.contactsEmptyDetail)),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// Shared editor: embedded (wide) or standalone ([ContactDetailScreen]).
class ContactDetailEditor extends StatefulWidget {
  const ContactDetailEditor({
    super.key,
    required this.contactId,
    required this.embedded,
    required this.onSaved,
    required this.onDeleted,
    this.groups = const <FrbContactGroupRow>[],
    this.groupMembers = const <int, List<FrbContactGroupMemberRow>>{},
    this.onCreated,
  });

  final int? contactId;
  final bool embedded;
  final VoidCallback onSaved;
  final VoidCallback onDeleted;
  final List<FrbContactGroupRow> groups;
  final Map<int, List<FrbContactGroupMemberRow>> groupMembers;
  /// Called with new row id after first save when [contactId] was null.
  final ValueChanged<int>? onCreated;

  @override
  State<ContactDetailEditor> createState() => _ContactDetailEditorState();
}

class _ContactDetailEditorState extends State<ContactDetailEditor> {
  bool _loading = true;
  FrbContactDetail? _detail;
  List<FrbContactRepositoryLinkRow> _repoLinks = <FrbContactRepositoryLinkRow>[];
  final TextEditingController _name = TextEditingController();
  final TextEditingController _notes = TextEditingController();
  final List<TextEditingController> _emailCtrls = <TextEditingController>[];
  final List<TextEditingController> _emailLabels = <TextEditingController>[];
  bool _shareOk = true;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  @override
  void didUpdateWidget(covariant ContactDetailEditor oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.contactId != widget.contactId) {
      unawaited(_load());
    }
  }

  Future<void> _load() async {
    if (widget.contactId == null) {
      setState(() {
        _loading = false;
        _detail = null;
        _name.clear();
        _notes.clear();
        _disposeEmailCtrls();
        _emailCtrls.add(TextEditingController());
        _emailLabels.add(TextEditingController());
        _shareOk = true;
        _repoLinks = <FrbContactRepositoryLinkRow>[];
      });
      return;
    }
    setState(() => _loading = true);
    try {
      final FrbContactDetail d = await frbContactsGet(contactId: _pi(widget.contactId!));
      final List<FrbContactRepositoryLinkRow> links =
          await frbContactsRepositoryLinksForContact(contactId: _pi(widget.contactId!));
      if (!mounted) {
        return;
      }
      _disposeEmailCtrls();
      _name.text = d.displayName;
      _notes.text = d.notes;
      _shareOk = d.externallyShareOk;
      for (final FrbContactEmailRow e in d.emails) {
        _emailCtrls.add(TextEditingController(text: e.email));
        _emailLabels.add(TextEditingController(text: e.label));
      }
      if (_emailCtrls.isEmpty) {
        _emailCtrls.add(TextEditingController());
        _emailLabels.add(TextEditingController());
      }
      setState(() {
        _detail = d;
        _repoLinks = links;
        _loading = false;
      });
    } catch (e, st) {
      appLogStderr('contact detail: $e\n$st');
      if (mounted) {
        setState(() => _loading = false);
      }
    }
  }

  void _disposeEmailCtrls() {
    for (final TextEditingController c in _emailCtrls) {
      c.dispose();
    }
    for (final TextEditingController c in _emailLabels) {
      c.dispose();
    }
    _emailCtrls.clear();
    _emailLabels.clear();
  }

  @override
  void dispose() {
    _name.dispose();
    _notes.dispose();
    _disposeEmailCtrls();
    super.dispose();
  }

  Set<int> _memberGroupIds(int contactId) {
    final Set<int> s = <int>{};
    for (final MapEntry<int, List<FrbContactGroupMemberRow>> e
        in widget.groupMembers.entries) {
      for (final FrbContactGroupMemberRow m in e.value) {
        if (_p64(m.id) == contactId) {
          s.add(e.key);
        }
      }
    }
    return s;
  }

  Future<void> _save(AppLocalizations l10n) async {
    final List<FrbContactEmailInput> emails = <FrbContactEmailInput>[];
    for (int i = 0; i < _emailCtrls.length; i++) {
      final String em = _emailCtrls[i].text.trim();
      if (em.isEmpty) {
        continue;
      }
      final String lab = i < _emailLabels.length ? _emailLabels[i].text.trim() : '';
      emails.add(FrbContactEmailInput(email: em, label: lab.isEmpty ? null : lab));
    }
    try {
      final FrbContactsRowId row = await frbContactsUpsert(
        u: FrbContactUpsert(
          id: widget.contactId != null ? _pi(widget.contactId!) : null,
          displayName: _name.text.trim(),
          notes: _notes.text,
          importOrigin: _detail?.importOrigin,
          externallyShareOk: _shareOk,
          emails: emails,
          pgpFingerprint: _detail?.pgpFingerprint,
          pgpKeyPath: _detail?.pgpKeyPath,
          smimeCertPath: _detail?.smimeCertPath,
          smimeNotes: _detail?.smimeNotes,
        ),
      );
      if (!mounted) {
        return;
      }
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(l10n.contactsSaved)));
      if (widget.contactId == null) {
        widget.onCreated?.call(_p64(row.id));
      }
      widget.onSaved();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _delete(AppLocalizations l10n) async {
    if (widget.contactId == null) {
      return;
    }
    final bool? ok = await showDialog<bool>(
      context: context,
      builder: (BuildContext ctx) => AlertDialog(
        title: Text(l10n.contactsDeleteSelectedTitle),
        content: Text(l10n.contactsDeleteSelectedBody(1)),
        actions: <Widget>[
          TextButton(onPressed: () => Navigator.pop(ctx, false), child: Text(l10n.cancel)),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: Text(l10n.delete),
          ),
        ],
      ),
    );
    if (ok != true || !mounted) {
      return;
    }
    try {
      await frbContactsDelete(contactId: _pi(widget.contactId!));
      widget.onDeleted();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _toggleRepo(int repositoryId, bool include) async {
    if (widget.contactId == null) {
      return;
    }
    try {
      await frbContactsSetRepositoryMembership(
        contactId: _pi(widget.contactId!),
        repositoryId: _pi(repositoryId),
        include: include,
      );
      final List<FrbContactRepositoryLinkRow> links =
          await frbContactsRepositoryLinksForContact(contactId: _pi(widget.contactId!));
      if (mounted) {
        setState(() => _repoLinks = links);
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _toggleGroup(int groupId, bool member) async {
    if (widget.contactId == null) {
      return;
    }
    try {
      if (member) {
        await frbContactsGroupAddMember(
          contactId: _pi(widget.contactId!),
          groupId: _pi(groupId),
        );
      } else {
        await frbContactsGroupRemoveMember(
          contactId: _pi(widget.contactId!),
          groupId: _pi(groupId),
        );
      }
      widget.onSaved();
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

    final EdgeInsets pad = widget.embedded
        ? const EdgeInsets.all(16)
        : const EdgeInsets.symmetric(horizontal: 16, vertical: 12);
    final int? cid = widget.contactId;

    return SingleChildScrollView(
      padding: pad,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            children: <Widget>[
              Expanded(
                child: TextField(
                  controller: _name,
                  decoration: InputDecoration(labelText: l10n.contactsDisplayName),
                ),
              ),
              FilledButton(
                onPressed: () => unawaited(_save(l10n)),
                child: Text(l10n.contactsSave),
              ),
              if (cid != null) ...<Widget>[
                const SizedBox(width: 8),
                IconButton(
                  icon: const Icon(Icons.delete_outline),
                  onPressed: () => unawaited(_delete(l10n)),
                ),
              ],
            ],
          ),
          TextField(
            controller: _notes,
            decoration: InputDecoration(labelText: l10n.contactsNotes),
            maxLines: 3,
          ),
          SwitchListTile(
            title: Text(l10n.contactsAllowExternalShare),
            value: _shareOk,
            onChanged: (bool v) => setState(() => _shareOk = v),
          ),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              icon: const Icon(Icons.add),
              label: Text(l10n.contactsAddEmail),
              onPressed: () {
                setState(() {
                  _emailCtrls.add(TextEditingController());
                  _emailLabels.add(TextEditingController());
                });
              },
            ),
          ),
          for (int i = 0; i < _emailCtrls.length; i++) ...<Widget>[
            Row(
              children: <Widget>[
                Expanded(
                  flex: 2,
                  child: TextField(
                    controller: _emailCtrls[i],
                    decoration: InputDecoration(
                      labelText: l10n.accountEmailAddressLabel,
                    ),
                    keyboardType: TextInputType.emailAddress,
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: TextField(
                    controller: _emailLabels[i],
                    decoration: InputDecoration(labelText: l10n.contactsEmailLabel),
                  ),
                ),
                IconButton(
                  icon: const Icon(Icons.close),
                  onPressed: _emailCtrls.length <= 1
                      ? null
                      : () {
                          setState(() {
                            _emailCtrls[i].dispose();
                            if (i < _emailLabels.length) {
                              _emailLabels[i].dispose();
                            }
                            _emailCtrls.removeAt(i);
                            if (i < _emailLabels.length) {
                              _emailLabels.removeAt(i);
                            }
                          });
                        },
                ),
              ],
            ),
          ],
          if (cid != null) ...<Widget>[
            const SizedBox(height: 16),
            Text(l10n.contactsRepositoryLinks, style: Theme.of(context).textTheme.titleSmall),
            for (final FrbContactRepositoryLinkRow link in _repoLinks)
              SwitchListTile(
                title: Text(link.name),
                subtitle: Text(link.kind),
                value: link.linked,
                onChanged: (bool v) => unawaited(_toggleRepo(_p64(link.repositoryId), v)),
              ),
            Text(l10n.contactsGroupMembership, style: Theme.of(context).textTheme.titleSmall),
            for (final FrbContactGroupRow g in widget.groups)
              SwitchListTile(
                title: Text(g.name),
                value: _memberGroupIds(cid).contains(_p64(g.id)),
                onChanged: (bool v) => unawaited(_toggleGroup(_p64(g.id), v)),
              ),
          ],
        ],
      ),
    );
  }
}
