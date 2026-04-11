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

import 'dart:convert';
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
  List<dynamic> _repos = <dynamic>[];
  List<dynamic> _groups = <dynamic>[];
  List<dynamic> _contacts = <dynamic>[];
  List<dynamic> _groupTargets = <dynamic>[];
  final Map<int, List<dynamic>> _groupMembers = <int, List<dynamic>>{};
  /// `null` means links not loaded yet for this contact.
  final Map<int, List<dynamic>?> _contactRepoLinks = <int, List<dynamic>?>{};

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
      final String r = await frbContactsRepositoriesList();
      final String g = await frbContactsGroupsList();
      final String c = await frbContactsListCompact(limit: _p(8000));
      final String tgt = await frbContactsGroupRepositoryTargetsList();
      if (!mounted) {
        return;
      }
      final List<dynamic> groups = jsonDecode(g) as List<dynamic>;
      final Map<int, List<dynamic>> gm = <int, List<dynamic>>{};
      for (final dynamic x in groups) {
        final Map<String, dynamic> m = x as Map<String, dynamic>;
        final int gid = (m['id'] as num).toInt();
        final String ms = await frbContactsGroupMembersList(groupId: _p(gid));
        gm[gid] = jsonDecode(ms) as List<dynamic>;
      }
      if (!mounted) {
        return;
      }
      setState(() {
        _repos = jsonDecode(r) as List<dynamic>;
        _groups = groups;
        _contacts = jsonDecode(c) as List<dynamic>;
        _groupTargets = jsonDecode(tgt) as List<dynamic>;
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
    for (final dynamic x in _repos) {
      final Map<String, dynamic> m = x as Map<String, dynamic>;
      if ((m['kind'] as String?) == 'platform') {
        return (m['id'] as num).toInt();
      }
    }
    return null;
  }

  bool _groupTargetsRepo(int groupId, int repositoryId) {
    for (final dynamic t in _groupTargets) {
      final Map<String, dynamic> m = t as Map<String, dynamic>;
      if ((m['groupId'] as num).toInt() == groupId &&
          (m['repositoryId'] as num).toInt() == repositoryId) {
        return true;
      }
    }
    return false;
  }

  Future<void> _addPlatformRepo(AppLocalizations l10n) async {
    try {
      final String j = await frbContactsRepositoryUpsert(
        json: jsonEncode(<String, dynamic>{
          'name': 'Platform',
          'kind': 'platform',
          'enabled': true,
        }),
      );
      final Map<String, dynamic> m = jsonDecode(j) as Map<String, dynamic>;
      appLogStderr('contacts: added platform repo id=${m['id']}');
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
        json: jsonEncode(<String, dynamic>{
          'name': name.text.trim().isEmpty ? 'CardDAV' : name.text.trim(),
          'kind': 'carddav',
          'enabled': true,
          'baseUrl': url.text.trim(),
          'collectionPath': coll.text.trim(),
        }),
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
    Map<String, dynamic> repo,
  ) async {
    final int id = (repo['id'] as num).toInt();
    final TextEditingController name = TextEditingController(
      text: repo['name'] as String? ?? '',
    );
    final TextEditingController base = TextEditingController(
      text: repo['baseUrl'] as String? ?? '',
    );
    final TextEditingController coll = TextEditingController(
      text: repo['collectionPath'] as String? ?? '',
    );
    bool enabled = repo['enabled'] as bool? ?? true;
    bool defaultNew = repo['defaultNewContact'] as bool? ?? false;
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
        json: jsonEncode(<String, dynamic>{
          'id': id,
          'name': name.text.trim(),
          'kind': repo['kind'],
          'enabled': enabled,
          'baseUrl': base.text.trim(),
          'collectionPath': coll.text.trim(),
          'defaultNewContact': defaultNew,
        }),
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
      final String out = await frbContactsCarddavPush(
        repositoryId: _p(repositoryId),
        username: user.text,
        password: pass.text,
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(out)));
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
      final String out = await frbContactsCarddavPull(
        repositoryId: _p(repositoryId),
        username: user.text,
        password: pass.text,
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(out)));
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
      final String out = await frbContactsImportVcardBytes(bytes: b);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(out)),
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
      final String vcf = await frbContactsExportVcard(contactIdsJson: '[]');
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
      await file.writeAsString(vcf);
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
      final List<Map<String, dynamic>> items = <Map<String, dynamic>>[];
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
        items.add(<String, dynamic>{
          'displayName': c.displayName,
          'emails': emails,
        });
      }
      final Map<String, dynamic> envelope = <String, dynamic>{'items': items};
      if (linkToPlatform && platformRid != null) {
        envelope['repositoryId'] = platformRid;
      }
      final String res = await frbContactsMergePlatformJson(
        payload: jsonEncode(envelope),
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(res)));
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
      await frbContactsGroupUpsert(json: jsonEncode(<String, dynamic>{'name': name}));
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
      final String s = await frbContactsApplyGroupRepositoryRules();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(s)));
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
      final String s = await frbContactsRepositoryLinksForContact(contactId: _p(contactId));
      if (!mounted) {
        return;
      }
      setState(() {
        _contactRepoLinks[contactId] = jsonDecode(s) as List<dynamic>;
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
    final List<int> memberIds = (_groupMembers[groupId] ?? <dynamic>[])
        .map((dynamic x) => (x as Map<String, dynamic>)['id'] as num)
        .map((num n) => n.toInt())
        .toList();
    final List<Map<String, dynamic>> choices = _contacts
        .map((dynamic x) => x as Map<String, dynamic>)
        .where((Map<String, dynamic> c) => !memberIds.contains((c['id'] as num).toInt()))
        .toList();
    if (choices.isEmpty) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('No contacts to add')),
        );
      }
      return;
    }
    int? pick = (choices.first['id'] as num).toInt();
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
                    (Map<String, dynamic> c) => DropdownMenuItem<int>(
                      value: (c['id'] as num).toInt(),
                      child: Text(
                        '${c['displayName'] ?? ''} ${c['primaryEmail'] ?? ''}'.trim(),
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
        ..._repos.map((dynamic r) {
          final Map<String, dynamic> m = r as Map<String, dynamic>;
          final int id = (m['id'] as num).toInt();
          final String kind = m['kind'] as String? ?? '';
          return ListTile(
            title: Text(m['name'] as String? ?? ''),
            subtitle: Text(
              <String>[
                kind,
                if (m['enabled'] == false) 'off',
                if (((m['syncError'] as String?) ?? '').isNotEmpty)
                  m['syncError'] as String,
              ].where((String s) => s.isNotEmpty).join(' · '),
            ),
            trailing: PopupMenuButton<String>(
              onSelected: (String v) async {
                if (v == 'edit') {
                  await _editRepositoryDialog(l10n, m);
                } else if (v == 'del') {
                  await _deleteRepositoryConfirm(
                    l10n,
                    id,
                    m['name'] as String? ?? '',
                  );
                } else if (v == 'sync') {
                  try {
                    final String s = await frbContactsSyncRepository(repositoryId: _p(id));
                    if (!context.mounted) {
                      return;
                    }
                    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(s)));
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
        ..._groups.map((dynamic g) {
          final Map<String, dynamic> gm = g as Map<String, dynamic>;
          final int gid = (gm['id'] as num).toInt();
          final List<dynamic> members = _groupMembers[gid] ?? <dynamic>[];
          return Card(
            margin: const EdgeInsets.only(bottom: 8),
            child: ExpansionTile(
              title: Text(gm['name'] as String? ?? ''),
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
                for (final dynamic repo in _repos)
                  Builder(
                    builder: (BuildContext ctx) {
                      final Map<String, dynamic> rm = repo as Map<String, dynamic>;
                      final int rid = (rm['id'] as num).toInt();
                      return SwitchListTile(
                        title: Text(rm['name'] as String? ?? ''),
                        subtitle: Text(rm['kind'] as String? ?? ''),
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
                ...members.map((dynamic mem) {
                  final Map<String, dynamic> mm = mem as Map<String, dynamic>;
                  final int cid = (mm['id'] as num).toInt();
                  return ListTile(
                    dense: true,
                    title: Text(mm['displayName'] as String? ?? ''),
                    subtitle: Text(mm['primaryEmail'] as String? ?? ''),
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
        ..._contacts.map((dynamic c) {
          final Map<String, dynamic> cm = c as Map<String, dynamic>;
          final int cid = (cm['id'] as num).toInt();
          final bool shareOk = cm['externallyShareOk'] as bool? ?? false;
          final String origin = cm['importOrigin'] as String? ?? '';
          final bool needsGate = !shareOk && origin == 'learned_from_mail';
          return Card(
            margin: const EdgeInsets.only(bottom: 6),
            child: ExpansionTile(
              onExpansionChanged: (bool ex) {
                if (ex) {
                  _loadRepoLinks(cid);
                }
              },
              title: Text(cm['displayName'] as String? ?? ''),
              subtitle: Text(
                '${cm['primaryEmail'] ?? ''} · $origin'.trim(),
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
                  ...(_contactRepoLinks[cid]!).map((dynamic lk) {
                    final Map<String, dynamic> lm = lk as Map<String, dynamic>;
                    final int rid = (lm['repositoryId'] as num).toInt();
                    final bool linked = lm['linked'] as bool? ?? false;
                    final bool canToggle = shareOk && !needsGate;
                    return SwitchListTile(
                      title: Text(lm['name'] as String? ?? ''),
                      subtitle: Text(lm['kind'] as String? ?? ''),
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
