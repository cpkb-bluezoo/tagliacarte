import 'package:flutter_quill/flutter_quill.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tagliacarte_ui/src/util/compose_reply.dart';
import 'package:tagliacarte_ui/src/widgets/rich_message_body_editor.dart';

void main() {
  test('sanitizeOutboundRichHtml removes script', () {
    expect(
      sanitizeOutboundRichHtml('<p>Hi</p><script>x</script>'),
      '<p>Hi</p>',
    );
  });

  test('exportRichTextBodySnapshot yields plain and html', () {
    final Document doc = Document.fromJson(<dynamic>[
      <String, dynamic>{'insert': 'Hello '},
      <String, dynamic>{
        'insert': 'world',
        'attributes': <String, dynamic>{'bold': true},
      },
      <String, dynamic>{'insert': '\n'},
    ]);
    final RichTextBodySnapshot s = exportRichTextBodySnapshot(doc);
    expect(s.plain, contains('Hello'));
    expect(s.plain, contains('world'));
    expect(s.html.toLowerCase(), contains('hello'));
    expect(s.html.toLowerCase(), contains('world'));
  });
}
