/// Locale-aware date and time formatting for `DATETIME()`, without `intl`.
///
/// Supports the Fluent options `dateStyle`, `timeStyle`, `weekday`, `year`,
/// `month`, `day`, `hour`, `minute`, `second` and `hour12`. Time zone names are
/// not rendered. Languages without data fall back to English.
library;

import '../locale/locale_tag.dart';

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
    required this.monthsShortStandalone,
    required this.monthsNarrow,
    required this.monthsNarrowStandalone,
    required this.weekdaysLong,
    required this.weekdaysShort,
    required this.weekdaysNarrow,
    required this.weekdaysNarrowStandalone,
    required this.numericOrder,
    required this.numericSeparator,
    required this.padNumeric,
    required this.twelveHour,
    required this.named,
    required this.dateTimeJoin,
    this.shortYear = '2-digit',
    this.padDay,
    this.amLabel = 'AM',
    this.pmLabel = 'PM',
  });

  /// Standalone month names (January ... December).
  final List<String> monthsLong;

  /// Month names used inside a full date (e.g. Russian `5 января`).
  final List<String> monthsGenitive;
  final List<String> monthsShort;
  final List<String> monthsShortStandalone;
  final List<String> monthsNarrow;
  final List<String> monthsNarrowStandalone;

  /// Weekday names, Monday first.
  final List<String> weekdaysLong;
  final List<String> weekdaysShort;
  final List<String> weekdaysNarrow;
  final List<String> weekdaysNarrowStandalone;

  /// Order of numeric date components: any of `d`, `M`, `y`.
  final List<String> numericOrder;
  final String numericSeparator;

  /// Default padding for multi-component numeric dates (`05.01.2026`).
  final bool padNumeric;
  /// Day-specific override, e.g. Polish `5.01.2026`.
  final bool? padDay;

  /// Width of the year in the locale's short date style.
  final String shortYear;

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

// Narrow/stand-alone names and short-year widths: CLDR 48.2, Gregorian
// ca-gregorian.json for the supported profiles. This is not a full CLDR matcher.
final Map<String, _DateLocale> _dateLocales = {
  'en': _DateLocale(
    monthsShortStandalone: const [
      'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun',
      'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec',
    ],
    monthsNarrow: const ['J', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O', 'N', 'D'],
    monthsNarrowStandalone: const ['J', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O', 'N', 'D'],
    weekdaysNarrow: const ['M', 'T', 'W', 'T', 'F', 'S', 'S'],
    weekdaysNarrowStandalone: const ['M', 'T', 'W', 'T', 'F', 'S', 'S'],
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
  'en-GB': _DateLocale(
    monthsShortStandalone: const [
      'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun',
      'Jul', 'Aug', 'Sept', 'Oct', 'Nov', 'Dec',
    ],
    monthsNarrow: const ['J', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O', 'N', 'D'],
    monthsNarrowStandalone: const ['J', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O', 'N', 'D'],
    weekdaysNarrow: const ['M', 'T', 'W', 'T', 'F', 'S', 'S'],
    weekdaysNarrowStandalone: const ['M', 'T', 'W', 'T', 'F', 'S', 'S'],
    shortYear: 'numeric',
    monthsLong: _enMonths,
    monthsGenitive: _enMonths,
    monthsShort: const [
      'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun',
      'Jul', 'Aug', 'Sept', 'Oct', 'Nov', 'Dec',
    ],
    weekdaysLong: _enDays,
    weekdaysShort: _enDaysShort,
    numericOrder: const ['d', 'M', 'y'],
    numericSeparator: '/',
    padNumeric: true,
    twelveHour: false,
    dateTimeJoin: ', ',
    named: (d, m, g, y) => _join([d, m, y], ' '),
  ),
  'ru': _DateLocale(
    monthsShortStandalone: const [
      'янв.', 'февр.', 'март', 'апр.', 'май', 'июнь',
      'июль', 'авг.', 'сент.', 'окт.', 'нояб.', 'дек.',
    ],
    monthsNarrow: const ['Я', 'Ф', 'М', 'А', 'М', 'И', 'И', 'А', 'С', 'О', 'Н', 'Д'],
    monthsNarrowStandalone: const ['Я', 'Ф', 'М', 'А', 'М', 'И', 'И', 'А', 'С', 'О', 'Н', 'Д'],
    weekdaysNarrow: const ['П', 'В', 'С', 'Ч', 'П', 'С', 'В'],
    weekdaysNarrowStandalone: const ['П', 'В', 'С', 'Ч', 'П', 'С', 'В'],
    shortYear: 'numeric',
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
    monthsShortStandalone: const [
      'січ.', 'лют.', 'бер.', 'квіт.', 'трав.', 'черв.',
      'лип.', 'серп.', 'вер.', 'жовт.', 'лист.', 'груд.',
    ],
    monthsNarrow: const ['с', 'л', 'б', 'к', 'т', 'ч', 'л', 'с', 'в', 'ж', 'л', 'г'],
    monthsNarrowStandalone: const ['С', 'Л', 'Б', 'К', 'Т', 'Ч', 'Л', 'С', 'В', 'Ж', 'Л', 'Г'],
    weekdaysNarrow: const ['П', 'В', 'С', 'Ч', 'П', 'С', 'Н'],
    weekdaysNarrowStandalone: const ['П', 'В', 'С', 'Ч', 'П', 'С', 'Н'],
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
    monthsShortStandalone: const [
      'Jan', 'Feb', 'Mär', 'Apr', 'Mai', 'Jun',
      'Jul', 'Aug', 'Sep', 'Okt', 'Nov', 'Dez',
    ],
    monthsNarrow: const ['J', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O', 'N', 'D'],
    monthsNarrowStandalone: const ['J', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O', 'N', 'D'],
    weekdaysNarrow: const ['M', 'D', 'M', 'D', 'F', 'S', 'S'],
    weekdaysNarrowStandalone: const ['M', 'D', 'M', 'D', 'F', 'S', 'S'],
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
    monthsShortStandalone: const [
      'janv.', 'févr.', 'mars', 'avr.', 'mai', 'juin',
      'juil.', 'août', 'sept.', 'oct.', 'nov.', 'déc.',
    ],
    monthsNarrow: const ['J', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O', 'N', 'D'],
    monthsNarrowStandalone: const ['J', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O', 'N', 'D'],
    weekdaysNarrow: const ['L', 'M', 'M', 'J', 'V', 'S', 'D'],
    weekdaysNarrowStandalone: const ['L', 'M', 'M', 'J', 'V', 'S', 'D'],
    shortYear: 'numeric',
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
    monthsShortStandalone: const [
      'ene', 'feb', 'mar', 'abr', 'may', 'jun',
      'jul', 'ago', 'sept', 'oct', 'nov', 'dic',
    ],
    monthsNarrow: const ['E', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O', 'N', 'D'],
    monthsNarrowStandalone: const ['E', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O', 'N', 'D'],
    weekdaysNarrow: const ['L', 'M', 'X', 'J', 'V', 'S', 'D'],
    weekdaysNarrowStandalone: const ['L', 'M', 'X', 'J', 'V', 'S', 'D'],
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
    monthsShortStandalone: const [
      'sty', 'lut', 'mar', 'kwi', 'maj', 'cze',
      'lip', 'sie', 'wrz', 'paź', 'lis', 'gru',
    ],
    monthsNarrow: const ['s', 'l', 'm', 'k', 'm', 'c', 'l', 's', 'w', 'p', 'l', 'g'],
    monthsNarrowStandalone: const ['S', 'L', 'M', 'K', 'M', 'C', 'L', 'S', 'W', 'P', 'L', 'G'],
    weekdaysNarrow: const ['p', 'w', 'ś', 'c', 'p', 's', 'n'],
    weekdaysNarrowStandalone: const ['P', 'W', 'Ś', 'C', 'P', 'S', 'N'],
    shortYear: 'numeric',
    padDay: false,
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
    named: (d, m, g, y) => _join([d, d == null ? m : g, y], ' '),
  ),
};

_DateLocale _localeData(String locale) {
  final tag = LocaleTag.tryParse(locale);
  if (tag == null) return _dateLocales['en']!;
  return _dateLocales[tag.toCatalogTag()] ??
      (tag.region == null ? null : _dateLocales['${tag.language}-${tag.region}']) ??
      _dateLocales[tag.language] ??
      _dateLocales['en']!;
}

const _numericWidths = {'numeric', '2-digit'};
const _textWidths = {'long', 'short', 'narrow'};
const _monthWidths = {'numeric', '2-digit', 'long', 'short', 'narrow'};
const _styles = {'short', 'medium', 'long', 'full'};

String? _supportedOption(String? value, Set<String> allowed) =>
    allowed.contains(value) ? value : null;

/// Formats [value] for [locale].
///
/// `dateStyle` / `timeStyle` (`short`, `medium`, `long`, `full`) map onto the
/// component options. Explicit component options take precedence over styles.
/// Invalid option strings are ignored before style expansion; with no valid
/// date or time components, the numeric date is shown. Years use Dart's signed
/// proleptic Gregorian numbering (there is no era option).
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
  dateStyle = _supportedOption(dateStyle, _styles);
  timeStyle = _supportedOption(timeStyle, _styles);
  weekday = _supportedOption(weekday, _textWidths);
  year = _supportedOption(year, _numericWidths);
  month = _supportedOption(month, _monthWidths);
  day = _supportedOption(day, _numericWidths);
  hour = _supportedOption(hour, _numericWidths);
  minute = _supportedOption(minute, _numericWidths);
  second = _supportedOption(second, _numericWidths);

  switch (dateStyle) {
    case 'short':
      year ??= data.shortYear;
      month ??= 'numeric';
      day ??= 'numeric';
      break;
    case 'medium':
      year ??= 'numeric';
      month ??= 'short';
      day ??= 'numeric';
      break;
    case 'long':
      year ??= 'numeric';
      month ??= 'long';
      day ??= 'numeric';
      break;
    case 'full':
      weekday ??= 'long';
      year ??= 'numeric';
      month ??= 'long';
      day ??= 'numeric';
      break;
  }
  switch (timeStyle) {
    case 'short':
      hour ??= 'numeric';
      minute ??= '2-digit';
      break;
    case 'medium':
    case 'long':
    case 'full':
      hour ??= 'numeric';
      minute ??= '2-digit';
      second ??= '2-digit';
      break;
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
    return _weekdayName(value, data, weekday, standalone: true);
  }

  final yearText = year == null
      ? null
      : (year == '2-digit'
          ? '${value.year < 0 ? '-' : ''}${_two(value.year.abs() % 100)}'
          : '${value.year}');
  final textualMonth = _textWidths.contains(month);

  String? core;
  if (textualMonth) {
    final index = value.month - 1;
    final String name;
    final String genitive;
    if (month == 'long') {
      name = data.monthsLong[index];
      genitive = data.monthsGenitive[index];
    } else if (month == 'short') {
      name = data.monthsShortStandalone[index];
      genitive = data.monthsShort[index];
    } else {
      name = data.monthsNarrowStandalone[index];
      genitive = data.monthsNarrow[index];
    }
    final contextualName = day == null ? name : genitive;
    final dayText = day == null
        ? null
        : (day == '2-digit' ? _two(value.day) : '${value.day}');
    core = data.named(dayText, contextualName, genitive, yearText);
  } else {
    final componentCount = [year, month, day].whereType<String>().length;
    final padDay = componentCount > 1 && (data.padDay ?? data.padNumeric);
    final padMonth = componentCount > 1 && data.padNumeric;
    final parts = <String, String?>{
      'd': day == null
          ? null
          : (day == '2-digit' || padDay ? _two(value.day) : '${value.day}'),
      'M': month == null
          ? null
          : (month == '2-digit' || padMonth
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

String _weekdayName(DateTime value, _DateLocale data, String style,
    {bool standalone = false}) {
  final names = style == 'narrow'
      ? (standalone ? data.weekdaysNarrowStandalone : data.weekdaysNarrow)
      : (style == 'short' ? data.weekdaysShort : data.weekdaysLong);
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
  if (minute != null) {
    parts.add(minute == '2-digit' || hour != null || second != null
        ? _two(value.minute) : '${value.minute}');
  }
  if (second != null) {
    parts.add(second == '2-digit' || hour != null || minute != null
        ? _two(value.second) : '${value.second}');
  }

  var text = parts.join(':');
  if (twelve && hour != null) {
    text = '$text ${rawHour < 12 ? data.amLabel : data.pmLabel}';
  }
  return text;
}
