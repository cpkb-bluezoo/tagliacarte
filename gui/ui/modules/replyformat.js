// The reply header line ("On $date at $time, $sender wrote:") and the date
// and time patterns offered for it. Patterns are the ICU skeletons the
// Flutter UI stored; the ones offered are handled with Intl, and a custom
// pattern is formatted token by token.

export const DATE_PATTERNS = ['', 'y-MM-dd', 'MM/dd/y', 'dd/MM/y', 'yMMMd', 'yMMMEd'];
export const TIME_PATTERNS = ['', 'jm', 'HH:mm', 'HH:mm:ss'];

export const DATE_PATTERN_LABELS = {
  '': 'replyDatePresetLocale',
  'y-MM-dd': 'replyDatePresetIso',
  'MM/dd/y': 'replyDatePresetUs',
  'dd/MM/y': 'replyDatePresetEu',
  yMMMd: 'replyDatePresetMedium',
  yMMMEd: 'replyDatePresetWeekday'
};

export const TIME_PATTERN_LABELS = {
  '': 'replyTimePresetLocale',
  jm: 'replyTimePreset12h',
  'HH:mm': 'replyTimePreset24h',
  'HH:mm:ss': 'replyTimePreset24hSeconds'
};

const pad = (n, width) => String(n).padStart(width, '0');

// Token by token: runs of one letter are a field, anything else is literal.
function formatPattern(pattern, d, locale) {
  return pattern.replace(/(y+|M+|d+|E+|H+|h+|m+|s+|a+)|('[^']*')/g, function (m) {
    if (m[0] === "'") { return m.slice(1, -1) || "'"; }
    const n = m.length;
    switch (m[0]) {
      case 'y': return n === 2 ? pad(d.getFullYear() % 100, 2) : String(d.getFullYear());
      case 'M':
        if (n <= 2) { return pad(d.getMonth() + 1, n); }
        return new Intl.DateTimeFormat(locale, { month: n === 3 ? 'short' : 'long' }).format(d);
      case 'd': return pad(d.getDate(), n);
      case 'E': return new Intl.DateTimeFormat(locale, { weekday: n <= 3 ? 'short' : 'long' }).format(d);
      case 'H': return pad(d.getHours(), n);
      case 'h': return pad(d.getHours() % 12 || 12, n);
      case 'm': return pad(d.getMinutes(), n);
      case 's': return pad(d.getSeconds(), n);
      case 'a': return new Intl.DateTimeFormat(locale, { hour: 'numeric', hour12: true })
        .formatToParts(d).filter(function (p) { return p.type === 'dayPeriod'; }).map(function (p) { return p.value; }).join('');
      default: return m;
    }
  });
}

export function formatReplyDate(d, pattern, locale) {
  switch (pattern) {
    case '': return new Intl.DateTimeFormat(locale, { dateStyle: 'long' }).format(d);
    case 'yMMMd': return new Intl.DateTimeFormat(locale, { year: 'numeric', month: 'short', day: 'numeric' }).format(d);
    case 'yMMMEd':
      return new Intl.DateTimeFormat(locale, { weekday: 'short', year: 'numeric', month: 'short', day: 'numeric' }).format(d);
    default: return formatPattern(pattern, d, locale);
  }
}

export function formatReplyTime(d, pattern, locale) {
  switch (pattern) {
    case '': return new Intl.DateTimeFormat(locale, { timeStyle: 'short' }).format(d);
    case 'jm': return new Intl.DateTimeFormat(locale, { hour: 'numeric', minute: '2-digit' }).format(d);
    default: return formatPattern(pattern, d, locale);
  }
}

export function expandReplyHeader(template, date, time, sender) {
  return template.split('$date').join(date).split('$time').join(time).split('$sender').join(sender);
}
