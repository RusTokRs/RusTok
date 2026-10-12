/// Locale-aware date and time formatting for `DATETIME()`, without `intl`.
///
/// Supports the Fluent options `dateStyle`, `timeStyle`, `weekday`, `year`,
/// `month`, `day`, `hour`, `minute`, `second` and `hour12`. Time zone names are
/// not rendered. Languages without data fall back to English.
library;

typedef _NamedDate = String Function(
  String? day,
  String month,
  String monthGenitive,
  String? year,
);

class _DateLocale {
  const _DateLocale({
    required this.monthsLong,
    required this.monthsGenitive,
    required this.monthsShort,
    required this.weekdaysLong,
    required this.weekdaysShort,
    required this.numericOrder,
    required this.numericSeparator,
    required this.padNumeric,
    required this.twelveHour,
    required this.named,
    required this.dateTimeJoin,
    this.amLabel = 'AM',
    this.pmLabel = 'PM',
  });

  /// Standalone month names (January ... December).
  final List<String> monthsLong;

  /// Month names used inside a full date (e.g. Russian `5 января`).
  final List<String> monthsGenitive;
  final List<String> monthsShort;

  /// Weekday names, Monday first.
  final List<String> weekdaysLong;
  final List<String> weekdaysShort;

  /// Order of numeric date components: any of `d`, `M`, `y`.
  final List<String> numericOrder;
  final String numericSeparator;

  /// Whether numeric day and month are zero-padded (`05.01.2026`).
  final bool padNumeric;

  final bool twelveHour;
  final String amLabel;
  final String pmLabel;
  final _NamedDate named;

  /// Separator between date and time.
  final String dateTimeJoin;
}

String _join(List<String?> parts, String separator) =>
    parts.whereType<String>().join(separator);

String _two(int value) => value.toString().padLeft(2, '0');

const List<String> _enMonths = [
  'January', 'February', 'March', 'April', 'May', 'June', 'July', 'August',
  'September', 'October', 'November', 'December',
];
const List<String> _enMonthsShort = [
  'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov',
  'Dec',
];
const List<String> _enDays = [
  'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday',
];
const List<String> _enDaysShort = [
  'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun',
];

final Map<String, _DateLocale> _dateLocales = {
  'en': _DateLocale(
    monthsLong: _enMonths,
    monthsGenitive: _enMonths,
    monthsShort: _enMonthsShort,
    weekdaysLong: _enDays,
    weekdaysShort: _enDaysShort,
    numericOrder: const ['M', 'd', 'y'],
    numericSeparator: '/',
    padNumeric: false,
    twelveHour: true,
    amLabel: 'AM',
    pmLabel: 'PM',
    dateTimeJoin: ', ',
    named: (d, m, g, y) => d == null
        ? _join([m, y], ' ')
        : _join(['$m $d', y], ', '),
  ),
  'ru': _DateLocale(
    monthsLong: const [
      'январь', 'февраль', 'март', 'апрель', 'май', 'июнь', 'июль', 'август',
      'сентябрь', 'октябрь', 'ноябрь', 'декабрь',
    ],
    monthsGenitive: const [
      'января', 'февраля', 'марта', 'апреля', 'мая', 'июня', 'июля',
      'августа', 'сентября', 'октября', 'ноября', 'декабря',
    ],
    monthsShort: const [
      'янв.', 'февр.', 'мар.', 'апр.', 'мая', 'июн.', 'июл.', 'авг.',
      'сент.', 'окт.', 'нояб.', 'дек.',
    ],
    weekdaysLong: const [
      'понедельник', 'вторник', 'среда', 'четверг', 'пятница', 'суббота',
      'воскресенье',
    ],
    weekdaysShort: const ['пн', 'вт', 'ср', 'чт', 'пт', 'сб', 'вс'],
    numericOrder: const ['d', 'M', 'y'],
    numericSeparator: '.',
    padNumeric: true,
    twelveHour: false,
    dateTimeJoin: ', ',
    named: (d, m, g, y) => d == null
        ? _join([m, y], ' ')
        : _join(['$d $g', y == null ? null : '$y г.'], ' '),
  ),
  'uk': _DateLocale(
    monthsLong: const [
      'січень', 'лютий', 'березень', 'квітень', 'травень', 'червень', 'липень',
      'серпень', 'вересень', 'жовтень', 'листопад', 'грудень',
    ],
    monthsGenitive: const [
      'січня', 'лютого', 'березня', 'квітня', 'травня', 'червня', 'липня',
      'серпня', 'вересня', 'жовтня', 'листопада', 'грудня',
    ],
    monthsShort: const [
      'січ.', 'лют.', 'бер.', 'квіт.', 'трав.', 'черв.', 'лип.', 'серп.',
      'вер.', 'жовт.', 'лист.', 'груд.',
    ],
    weekdaysLong: const [
      'понеділок', 'вівторок', 'середа', 'четвер', "п'ятниця", 'субота',
      'неділя',
    ],
    weekdaysShort: const ['пн', 'вт', 'ср', 'чт', 'пт', 'сб', 'нд'],
    numericOrder: const ['d', 'M', 'y'],
    numericSeparator: '.',
    padNumeric: true,
    twelveHour: false,
    dateTimeJoin: ', ',
    named: (d, m, g, y) => d == null
        ? _join([m, y], ' ')
        : _join(['$d $g', y == null ? null : '$y р.'], ' '),
  ),
  'de': _DateLocale(
    monthsLong: const [
      'Januar', 'Februar', 'März', 'April', 'Mai', 'Juni', 'Juli', 'August',
      'September', 'Oktober', 'November', 'Dezember',
    ],
    monthsGenitive: const [
      'Januar', 'Februar', 'März', 'April', 'Mai', 'Juni', 'Juli', 'August',
      'September', 'Oktober', 'November', 'Dezember',
    ],
    monthsShort: const [
      'Jan.', 'Feb.', 'März', 'Apr.', 'Mai', 'Juni', 'Juli', 'Aug.', 'Sept.',
      'Okt.', 'Nov.', 'Dez.',
    ],
    weekdaysLong: const [
      'Montag', 'Dienstag', 'Mittwoch', 'Donnerstag', 'Freitag', 'Samstag',
      'Sonntag',
    ],
    weekdaysShort: const ['Mo.', 'Di.', 'Mi.', 'Do.', 'Fr.', 'Sa.', 'So.'],
    numericOrder: const ['d', 'M', 'y'],
    numericSeparator: '.',
    padNumeric: true,
    twelveHour: false,
    dateTimeJoin: ', ',
    named: (d, m, g, y) => _join([d == null ? null : '$d.', m, y], ' '),
  ),
  'fr': _DateLocale(
    monthsLong: const [
      'janvier', 'février', 'mars', 'avril', 'mai', 'juin', 'juillet', 'août',
      'septembre', 'octobre', 'novembre', 'décembre',
    ],
    monthsGenitive: const [
      'janvier', 'février', 'mars', 'avril', 'mai', 'juin', 'juillet', 'août',
      'septembre', 'octobre', 'novembre', 'décembre',
    ],
    monthsShort: const [
      'janv.', 'févr.', 'mars', 'avr.', 'mai', 'juin', 'juil.', 'août',
      'sept.', 'oct.', 'nov.', 'déc.',
    ],
    weekdaysLong: const [
      'lundi', 'mardi', 'mercredi', 'jeudi', 'vendredi', 'samedi', 'dimanche',
    ],
    weekdaysShort: const ['lun.', 'mar.', 'mer.', 'jeu.', 'ven.', 'sam.', 'dim.'],
    numericOrder: const ['d', 'M', 'y'],
    numericSeparator: '/',
    padNumeric: true,
    twelveHour: false,
    dateTimeJoin: ' ',
    named: (d, m, g, y) => _join([d, m, y], ' '),
  ),
  'es': _DateLocale(
    monthsLong: const [
      'enero', 'febrero', 'marzo', 'abril', 'mayo', 'junio', 'julio', 'agosto',
      'septiembre', 'octubre', 'noviembre', 'diciembre',
    ],
    monthsGenitive: const [
      'enero', 'febrero', 'marzo', 'abril', 'mayo', 'junio', 'julio', 'agosto',
      'septiembre', 'octubre', 'noviembre', 'diciembre',
    ],
    monthsShort: const [
      'ene', 'feb', 'mar', 'abr', 'may', 'jun', 'jul', 'ago', 'sept', 'oct',
      'nov', 'dic',
    ],
    weekdaysLong: const [
      'lunes', 'martes', 'miércoles', 'jueves', 'viernes', 'sábado', 'domingo',
    ],
    weekdaysShort: const ['lun', 'mar', 'mié', 'jue', 'vie', 'sáb', 'dom'],
    numericOrder: const ['d', 'M', 'y'],
    numericSeparator: '/',
    padNumeric: false,
    twelveHour: false,
    dateTimeJoin: ', ',
    named: (d, m, g, y) => d == null
        ? _join([m, y], ' de ')
        : _join([d, m, y], ' de '),
  ),
  'pl': _DateLocale(
    monthsLong: const [
      'styczeń', 'luty', 'marzec', 'kwiecień', 'maj', 'czerwiec', 'lipiec',
      'sierpień', 'wrzesień', 'październik', 'listopad', 'grudzień',
    ],
    monthsGenitive: const [
      'stycznia', 'lutego', 'marca', 'kwietnia', 'maja', 'czerwca', 'lipca',
      'sierpnia', 'września', 'października', 'listopada', 'grudnia',
    ],
    monthsShort: const [
      'sty', 'lut', 'mar', 'kwi', 'maj', 'cze', 'lip', 'sie', 'wrz', 'paź',
      'lis', 'gru',
    ],
    weekdaysLong: const [
      'poniedziałek', 'wtorek', 'środa', 'czwartek', 'piątek', 'sobota',
      'niedziela',
    ],
    weekdaysShort: const [
      'pon.', 'wt.', 'śr.', 'czw.', 'pt.', 'sob.', 'niedz.',
    ],
    numericOrder: const ['d', 'M', 'y'],
    numericSeparator: '.',
    padNumeric: true,
    twelveHour: false,
    dateTimeJoin: ', ',
    named: (d, m, g, y) => _join([d, g, y], ' '),
  ),
};

_DateLocale _localeData(String locale) {
  final language = locale.split(RegExp(r'[-_]')).first.toLowerCase();
  return _dateLocales[language] ?? _dateLocales['en']!;
}

/// Formats [value] for [locale].
///
/// `dateStyle` / `timeStyle` (`short`, `medium`, `long`, `full`) map onto the
/// component options. Explicit component options take precedence over styles.
/// With no date or time options, the numeric date is shown.
String formatFluentDateTime(
  DateTime value, {
  required String locale,
  String? dateStyle,
  String? timeStyle,
  String? weekday,
  String? year,
  String? month,
  String? day,
  String? hour,
  String? minute,
  String? second,
  bool? hour12,
}) {
  final data = _localeData(locale);

  switch (dateStyle) {
    case 'short':
      year ??= '2-digit';
      month ??= 'numeric';
      day ??= 'numeric';
    case 'medium':
      year ??= 'numeric';
      month ??= 'short';
      day ??= 'numeric';
    case 'long':
      year ??= 'numeric';
      month ??= 'long';
      day ??= 'numeric';
    case 'full':
      weekday ??= 'long';
      year ??= 'numeric';
      month ??= 'long';
      day ??= 'numeric';
  }
  switch (timeStyle) {
    case 'short':
      hour ??= 'numeric';
      minute ??= '2-digit';
    case 'medium':
    case 'long':
    case 'full':
      hour ??= 'numeric';
      minute ??= '2-digit';
      second ??= '2-digit';
  }

  final hasDate =
      year != null || month != null || day != null || weekday != null;
  final hasTime = hour != null || minute != null || second != null;
  if (!hasDate && !hasTime) {
    year = 'numeric';
    month = 'numeric';
    day = 'numeric';
  }

  final datePart = _datePart(
    value,
    data,
    weekday: weekday,
    year: year,
    month: month,
    day: day,
  );
  final timePart = _timePart(
    value,
    data,
    hour: hour,
    minute: minute,
    second: second,
    hour12: hour12,
  );

  if (datePart != null && timePart != null) {
    return '$datePart${data.dateTimeJoin}$timePart';
  }
  return datePart ?? timePart ?? '';
}

String? _datePart(
  DateTime value,
  _DateLocale data, {
  String? weekday,
  String? year,
  String? month,
  String? day,
}) {
  if (year == null && month == null && day == null) {
    // Weekday alone: just the name.
    if (weekday == null) return null;
    return _weekdayName(value, data, weekday);
  }

  final yearText = year == null
      ? null
      : (year == '2-digit' ? _two(value.year % 100) : '${value.year}');
  final textualMonth = month == 'long' || month == 'short';

  String? core;
  if (textualMonth) {
    final index = value.month - 1;
    final name = month == 'long' ? data.monthsLong[index] : data.monthsShort[index];
    final genitive =
        month == 'long' ? data.monthsGenitive[index] : data.monthsShort[index];
    final dayText = day == null
        ? null
        : (day == '2-digit' ? _two(value.day) : '${value.day}');
    core = data.named(dayText, name, genitive, yearText);
  } else {
    final padded = data.padNumeric;
    final parts = <String, String?>{
      'd': day == null
          ? null
          : (day == '2-digit' || padded ? _two(value.day) : '${value.day}'),
      'M': month == null
          ? null
          : (month == '2-digit' || padded
              ? _two(value.month)
              : '${value.month}'),
      'y': yearText,
    };
    core = _join(
      [for (final key in data.numericOrder) parts[key]],
      data.numericSeparator,
    );
    if (core.isEmpty) core = null;
  }

  if (weekday == null) return core;
  final name = _weekdayName(value, data, weekday);
  return core == null ? name : '$name, $core';
}

String _weekdayName(DateTime value, _DateLocale data, String style) {
  final names = style == 'short' ? data.weekdaysShort : data.weekdaysLong;
  return names[value.weekday - 1];
}

String? _timePart(
  DateTime value,
  _DateLocale data, {
  String? hour,
  String? minute,
  String? second,
  bool? hour12,
}) {
  if (hour == null && minute == null && second == null) return null;

  final twelve = hour12 ?? data.twelveHour;
  final rawHour = value.hour;
  final displayHour =
      twelve ? (rawHour % 12 == 0 ? 12 : rawHour % 12) : rawHour;

  final parts = <String>[];
  if (hour != null) {
    // 24-hour clocks always use two digits; 12-hour clocks do not, unless asked.
    parts.add(!twelve || hour == '2-digit'
        ? _two(displayHour)
        : '$displayHour');
  }
  if (minute != null) parts.add(_two(value.minute));
  if (second != null) parts.add(_two(value.second));

  var text = parts.join(':');
  if (twelve && hour != null) {
    text = '$text ${rawHour < 12 ? data.amLabel : data.pmLabel}';
  }
  return text;
}
