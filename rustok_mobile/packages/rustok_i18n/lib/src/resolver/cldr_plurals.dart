// The CLDR cardinal plural table below is generated from
// `cldr-core@48.2.0/supplemental/plurals.json` and validated against the
// sample values shipped in that file:
//
//   python3 rustok_mobile/tooling/scripts/verify_cldr_plurals.py \
//       --cldr-core /path/to/cldr-core/package
//   # 9570 samples checked, 0 mismatches, 0 structural problems
//
// The verifier also fails when a case body can complete normally: Dart rejects
// an implicit fall-through, and such a body would otherwise run the *next*
// rule (that is how the 34 `other`-only languages once followed Breton).
// Regenerate the table rather than editing the rules by hand.
/// Resolves the CLDR plural category (`zero`, `one`, `two`, `few`, `many`,
/// `other`) for a number and a BCP-47 language tag.
///
/// The table below is the CLDR 48 cardinal rule set
/// (`cldr-core@48.2.0/supplemental/plurals.json`), one `case` group per distinct
/// CLDR rule shape, with the rule text rewritten into Dart conditionals:
///
/// * `n` is the absolute value, `i` its integer part, `v` the number of visible
///   fraction digits and `f` their value.
/// * CLDR's `t` (fraction digits without trailing zeros) is used as `f`, and `w`
///   as `v`. The shortest decimal form of a Dart `num` never ends in a zero, so
///   the operands agree for every value a `num` can represent.
/// * CLDR's `e` (compact decimal exponent) is folded to `0`: the package never
///   formats in compact notation.
/// * Tags CLDR publishes no cardinal rule for resolve to `other`.
///
/// Limitations: `NaN` and infinities resolve to `other`; a trailing zero in the
/// caller's formatting is invisible here, so `1.0` counts as the integer `1`
/// (Slavic rules select `other` only for a *visible* fraction — format such a
/// value with `NUMBER()`/`minimumFractionDigits` if the distinction matters);
/// and values printed in exponential notation count as having no visible
/// fraction digits.
String resolveCldrPluralCategory(String language, num value) {
  // NaN and infinities have no plural category; never call toInt() on them.
  if (!value.isFinite) return 'other';

  final normalized = language.trim().toLowerCase().replaceAll('_', '-');
  // CLDR publishes a `pt-PT` rule that differs from `pt`; every other region or
  // script variant shares the rule of its primary language subtag.
  final lang = normalized == 'pt-pt' ? normalized : normalized.split('-').first;

  final n = value.abs();

  // CLDR operands. `i` is kept as a double so that very large values stay
  // exact, and the moduli are safe for any finite input.
  final v = _visibleFractionDigits(n);
  final f = _fractionDigits(n);
  final i = n - (n % 1);
  final n10 = n % 10;
  final n100 = n % 100;
  final n1000 = n % 1000;
  final n100000 = n % 100000;
  final n1000000 = n % 1000000;
  final i10 = (i % 10).toInt();
  final i100 = (i % 100).toInt();
  final i1000000 = (i % 1000000).toInt();
  final f10 = f % 10;
  final f100 = f % 100;

  switch (lang) {
    // one: n=1
    case 'af':
    case 'an':
    case 'asa':
    case 'az':
    case 'bal':
    case 'bem':
    case 'bez':
    case 'bg':
    case 'brx':
    case 'ce':
    case 'cgg':
    case 'chr':
    case 'ckb':
    case 'dv':
    case 'ee':
    case 'el':
    case 'eo':
    case 'eu':
    case 'fo':
    case 'fur':
    case 'gsw':
    case 'ha':
    case 'haw':
    case 'hu':
    case 'jgo':
    case 'jmc':
    case 'ka':
    case 'kaj':
    case 'kcg':
    case 'kk':
    case 'kkj':
    case 'kl':
    case 'ks':
    case 'ksb':
    case 'ku':
    case 'ky':
    case 'lb':
    case 'lg':
    case 'mas':
    case 'mgo':
    case 'ml':
    case 'mn':
    case 'mr':
    case 'nah':
    case 'nb':
    case 'nd':
    case 'ne':
    case 'nn':
    case 'nnh':
    case 'no':
    case 'nr':
    case 'ny':
    case 'nyn':
    case 'om':
    case 'or':
    case 'os':
    case 'pap':
    case 'ps':
    case 'rm':
    case 'rof':
    case 'rwk':
    case 'saq':
    case 'sd':
    case 'sdh':
    case 'seh':
    case 'sn':
    case 'so':
    case 'sq':
    case 'ss':
    case 'ssy':
    case 'st':
    case 'syr':
    case 'ta':
    case 'te':
    case 'teo':
    case 'tig':
    case 'tk':
    case 'tn':
    case 'tr':
    case 'ts':
    case 'ug':
    case 'uz':
    case 've':
    case 'vo':
    case 'vun':
    case 'wae':
    case 'xh':
    case 'xog':
      if (n == 1) return 'one';
      return 'other';

    // one: n=0..1
    case 'ak':
    case 'bho':
    case 'csw':
    case 'guw':
    case 'ln':
    case 'mg':
    case 'nso':
    case 'pa':
    case 'ti':
    case 'wa':
      if (_inRange(n, 0, 1)) return 'one';
      return 'other';

    // one: i=0orn=1
    case 'am':
    case 'as':
    case 'bn':
    case 'doi':
    case 'fa':
    case 'gu':
    case 'hi':
    case 'kn':
    case 'kok':
    case 'pcm':
    case 'zu':
      if ((i == 0) ||
          (n == 1)) return 'one';
      return 'other';

    // zero: n=0; one: n=1; two: n=2; few: n%100=3..10; many: n%100=11..99
    case 'ar':
    case 'ars':
      if (n == 0) return 'zero';
      if (n == 1) return 'one';
      if (n == 2) return 'two';
      if (_inRange(n100, 3, 10)) return 'few';
      if (_inRange(n100, 11, 99)) return 'many';
      return 'other';

    // one: i=1andv=0
    case 'ast':
    case 'de':
    case 'en':
    case 'et':
    case 'fi':
    case 'fy':
    case 'gl':
    case 'ia':
    case 'ie':
    case 'io':
    case 'lij':
    case 'nl':
    case 'sc':
    case 'sv':
    case 'sw':
    case 'ur':
    case 'yi':
      if (i == 1 && v == 0) return 'one';
      return 'other';

    // one: n%10=1andn%100!=11; few: n%10=2..4andn%100!=12..14; many: n%10=0orn%10=5..9orn%100=11..14
    case 'be':
      if (n10 == 1 && n100 != 11) return 'one';
      if (_inRange(n10, 2, 4) && !_inRange(n100, 12, 14)) return 'few';
      if ((n10 == 0) ||
          (_inRange(n10, 5, 9)) ||
          (_inRange(n100, 11, 14))) return 'many';
      return 'other';

    // zero: n=0; one: n=1
    case 'blo':
    case 'cv':
    case 'ksh':
      if (n == 0) return 'zero';
      if (n == 1) return 'one';
      return 'other';

    // No categories: every value resolves to `other`.
    case 'bm':
    case 'bo':
    case 'dz':
    case 'hnj':
    case 'id':
    case 'ig':
    case 'ii':
    case 'ja':
    case 'jbo':
    case 'jv':
    case 'jw':
    case 'kde':
    case 'kea':
    case 'km':
    case 'ko':
    case 'lkt':
    case 'lo':
    case 'ms':
    case 'my':
    case 'nqo':
    case 'osa':
    case 'sah':
    case 'ses':
    case 'sg':
    case 'su':
    case 'th':
    case 'to':
    case 'tpi':
    case 'und':
    case 'vi':
    case 'wo':
    case 'yo':
    case 'yue':
    case 'zh':
      return 'other';

    // one: n%10=1andn%100!=11,71,91; two: n%10=2andn%100!=12,72,92; few: n%10=3..4,9andn%100!=10..19,70..79,90..99; many: n!=0andn%1000000=0
    case 'br':
      if (n10 == 1 && !(n100 == 11 || n100 == 71 || n100 == 91)) return 'one';
      if (n10 == 2 && !(n100 == 12 || n100 == 72 || n100 == 92)) return 'two';
      if ((_inRange(n10, 3, 4) || n10 == 9) && !(_inRange(n100, 10, 19) || _inRange(n100, 70, 79) || _inRange(n100, 90, 99))) return 'few';
      if (n != 0 && n1000000 == 0) return 'many';
      return 'other';

    // one: v=0andi%10=1andi%100!=11orf%10=1andf%100!=11; few: v=0andi%10=2..4andi%100!=12..14orf%10=2..4andf%100!=12..14
    case 'bs':
    case 'hr':
    case 'sh':
    case 'sr':
      if ((v == 0 && i10 == 1 && i100 != 11) ||
          (f10 == 1 && f100 != 11)) return 'one';
      if ((v == 0 && _inRange(i10, 2, 4) && !_inRange(i100, 12, 14)) ||
          (_inRange(f10, 2, 4) && !_inRange(f100, 12, 14))) return 'few';
      return 'other';

    // one: i=1andv=0; many: e=0andi!=0andi%1000000=0andv=0ore!=0..5
    case 'ca':
    case 'it':
    case 'lld':
    case 'pt-pt':
    case 'scn':
    case 'vec':
      if (i == 1 && v == 0) return 'one';
      if (i != 0 && i1000000 == 0 && v == 0) return 'many';
      return 'other';

    // one: v=0andi=1,2,3orv=0andi%10!=4,6,9orv!=0andf%10!=4,6,9
    case 'ceb':
    case 'fil':
    case 'tl':
      if ((v == 0 && (i == 1 || i == 2 || i == 3)) ||
          (v == 0 && !(i10 == 4 || i10 == 6 || i10 == 9)) ||
          (v != 0 && !(f10 == 4 || f10 == 6 || f10 == 9))) return 'one';
      return 'other';

    // one: i=1andv=0; few: i=2..4andv=0; many: v!=0
    case 'cs':
    case 'sk':
      if (i == 1 && v == 0) return 'one';
      if (_inRange(i, 2, 4) && v == 0) return 'few';
      if (v != 0) return 'many';
      return 'other';

    // zero: n=0; one: n=1; two: n=2; few: n=3; many: n=6
    case 'cy':
      if (n == 0) return 'zero';
      if (n == 1) return 'one';
      if (n == 2) return 'two';
      if (n == 3) return 'few';
      if (n == 6) return 'many';
      return 'other';

    // one: n=1ort!=0andi=0,1
    case 'da':
      if ((n == 1) ||
          (f != 0 && (i == 0 || i == 1))) return 'one';
      return 'other';

    // one: v=0andi%100=1orf%100=1; two: v=0andi%100=2orf%100=2; few: v=0andi%100=3..4orf%100=3..4
    case 'dsb':
    case 'hsb':
      if ((v == 0 && i100 == 1) ||
          (f100 == 1)) return 'one';
      if ((v == 0 && i100 == 2) ||
          (f100 == 2)) return 'two';
      if ((v == 0 && _inRange(i100, 3, 4)) ||
          (_inRange(f100, 3, 4))) return 'few';
      return 'other';

    // one: n=1; many: e=0andi!=0andi%1000000=0andv=0ore!=0..5
    case 'es':
      if (n == 1) return 'one';
      if (i != 0 && i1000000 == 0 && v == 0) return 'many';
      return 'other';

    // one: i=0,1
    case 'ff':
    case 'hy':
    case 'kab':
      if ((i == 0 || i == 1)) return 'one';
      return 'other';

    // one: i=0,1; many: e=0andi!=0andi%1000000=0andv=0ore!=0..5
    case 'fr':
      if ((i == 0 || i == 1)) return 'one';
      if (i != 0 && i1000000 == 0 && v == 0) return 'many';
      return 'other';

    // one: n=1; two: n=2; few: n=3..6; many: n=7..10
    case 'ga':
      if (n == 1) return 'one';
      if (n == 2) return 'two';
      if (_inRange(n, 3, 6)) return 'few';
      if (_inRange(n, 7, 10)) return 'many';
      return 'other';

    // one: n=1,11; two: n=2,12; few: n=3..10,13..19
    case 'gd':
      if ((n == 1 || n == 11)) return 'one';
      if ((n == 2 || n == 12)) return 'two';
      if ((_inRange(n, 3, 10) || _inRange(n, 13, 19))) return 'few';
      return 'other';

    // one: v=0andi%10=1; two: v=0andi%10=2; few: v=0andi%100=0,20,40,60,80; many: v!=0
    case 'gv':
      if (v == 0 && i10 == 1) return 'one';
      if (v == 0 && i10 == 2) return 'two';
      if (v == 0 && (i100 == 0 || i100 == 20 || i100 == 40 || i100 == 60 || i100 == 80)) return 'few';
      if (v != 0) return 'many';
      return 'other';

    // one: i=1andv=0ori=0andv!=0; two: i=2andv=0
    case 'he':
      if ((i == 1 && v == 0) ||
          (i == 0 && v != 0)) return 'one';
      if (i == 2 && v == 0) return 'two';
      return 'other';

    // one: t=0andi%10=1andi%100!=11ort%10=1andt%100!=11
    case 'is':
      if ((f == 0 && i10 == 1 && i100 != 11) ||
          (f10 == 1 && f100 != 11)) return 'one';
      return 'other';

    // one: n=1; two: n=2
    case 'iu':
    case 'naq':
    case 'sat':
    case 'se':
    case 'sma':
    case 'smi':
    case 'smj':
    case 'smn':
    case 'sms':
      if (n == 1) return 'one';
      if (n == 2) return 'two';
      return 'other';

    // zero: n=0; one: n=1; two: n%100=2,22,42,62,82orn%1000=0andn%100000=1000..20000,40000,60000,80000orn!=0andn%1000000=100000; few: n%100=3,23,43,63,83; many: n!=1andn%100=1,21,41,61,81
    case 'kw':
      if (n == 0) return 'zero';
      if (n == 1) return 'one';
      if (((n100 == 2 || n100 == 22 || n100 == 42 || n100 == 62 || n100 == 82)) ||
          (n1000 == 0 && (_inRange(n100000, 1000, 20000) || n100000 == 40000 || n100000 == 60000 || n100000 == 80000)) ||
          (n != 0 && n1000000 == 100000)) return 'two';
      if ((n100 == 3 || n100 == 23 || n100 == 43 || n100 == 63 || n100 == 83)) return 'few';
      if (n != 1 && (n100 == 1 || n100 == 21 || n100 == 41 || n100 == 61 || n100 == 81)) return 'many';
      return 'other';

    // zero: n=0; one: i=0,1andn!=0
    case 'lag':
      if (n == 0) return 'zero';
      if ((i == 0 || i == 1) && n != 0) return 'one';
      return 'other';

    // one: n%10=1andn%100!=11..19; few: n%10=2..9andn%100!=11..19; many: f!=0
    case 'lt':
      if (n10 == 1 && !_inRange(n100, 11, 19)) return 'one';
      if (_inRange(n10, 2, 9) && !_inRange(n100, 11, 19)) return 'few';
      if (f != 0) return 'many';
      return 'other';

    // zero: n%10=0orn%100=11..19orv=2andf%100=11..19; one: n%10=1andn%100!=11orv=2andf%10=1andf%100!=11orv!=2andf%10=1
    case 'lv':
    case 'prg':
      if ((n10 == 0) ||
          (_inRange(n100, 11, 19)) ||
          (v == 2 && _inRange(f100, 11, 19))) return 'zero';
      if ((n10 == 1 && n100 != 11) ||
          (v == 2 && f10 == 1 && f100 != 11) ||
          (v != 2 && f10 == 1)) return 'one';
      return 'other';

    // one: v=0andi%10=1andi%100!=11orf%10=1andf%100!=11
    case 'mk':
      if ((v == 0 && i10 == 1 && i100 != 11) ||
          (f10 == 1 && f100 != 11)) return 'one';
      return 'other';

    // one: i=1andv=0; few: v!=0orn=0orn!=1andn%100=1..19
    case 'mo':
    case 'ro':
      if (i == 1 && v == 0) return 'one';
      if ((v != 0) ||
          (n == 0) ||
          (n != 1 && _inRange(n100, 1, 19))) return 'few';
      return 'other';

    // one: n=1; two: n=2; few: n=0orn%100=3..10; many: n%100=11..19
    case 'mt':
      if (n == 1) return 'one';
      if (n == 2) return 'two';
      if ((n == 0) ||
          (_inRange(n100, 3, 10))) return 'few';
      if (_inRange(n100, 11, 19)) return 'many';
      return 'other';

    // one: i=1andv=0; few: v=0andi%10=2..4andi%100!=12..14; many: v=0andi!=1andi%10=0..1orv=0andi%10=5..9orv=0andi%100=12..14
    case 'pl':
      if (i == 1 && v == 0) return 'one';
      if (v == 0 && _inRange(i10, 2, 4) && !_inRange(i100, 12, 14)) return 'few';
      if ((v == 0 && i != 1 && _inRange(i10, 0, 1)) ||
          (v == 0 && _inRange(i10, 5, 9)) ||
          (v == 0 && _inRange(i100, 12, 14))) return 'many';
      return 'other';

    // one: i=0..1; many: e=0andi!=0andi%1000000=0andv=0ore!=0..5
    case 'pt':
      if (_inRange(i, 0, 1)) return 'one';
      if (i != 0 && i1000000 == 0 && v == 0) return 'many';
      return 'other';

    // one: v=0andi%10=1andi%100!=11; few: v=0andi%10=2..4andi%100!=12..14; many: v=0andi%10=0orv=0andi%10=5..9orv=0andi%100=11..14
    case 'ru':
    case 'uk':
      if (v == 0 && i10 == 1 && i100 != 11) return 'one';
      if (v == 0 && _inRange(i10, 2, 4) && !_inRange(i100, 12, 14)) return 'few';
      if ((v == 0 && i10 == 0) ||
          (v == 0 && _inRange(i10, 5, 9)) ||
          (v == 0 && _inRange(i100, 11, 14))) return 'many';
      return 'other';

    // one: n%10=1andn%100!=11; two: n=2; few: n!=2andn%10=2..9andn%100!=11..19; many: f!=0
    case 'sgs':
      if (n10 == 1 && n100 != 11) return 'one';
      if (n == 2) return 'two';
      if (n != 2 && _inRange(n10, 2, 9) && !_inRange(n100, 11, 19)) return 'few';
      if (f != 0) return 'many';
      return 'other';

    // one: i=0orn=1; few: n=2..10
    case 'shi':
      if ((i == 0) ||
          (n == 1)) return 'one';
      if (_inRange(n, 2, 10)) return 'few';
      return 'other';

    // one: n=0,1ori=0andf=1
    case 'si':
      if (((n == 0 || n == 1)) ||
          (i == 0 && f == 1)) return 'one';
      return 'other';

    // one: v=0andi%100=1; two: v=0andi%100=2; few: v=0andi%100=3..4orv!=0
    case 'sl':
      if (v == 0 && i100 == 1) return 'one';
      if (v == 0 && i100 == 2) return 'two';
      if ((v == 0 && _inRange(i100, 3, 4)) ||
          (v != 0)) return 'few';
      return 'other';

    // one: n=0..1orn=11..99
    case 'tzm':
      if ((_inRange(n, 0, 1)) ||
          (_inRange(n, 11, 99))) return 'one';
      return 'other';
    default:
      // CLDR has no cardinal rule for this tag: no plural variation.
      return 'other';
  }
}

/// True when [value] is one of the integers in `from..to`.
///
/// CLDR compares a modulo result against a *set* of integers, so `5.5` is not
/// in `5..9` even though it lies between the bounds.
bool _inRange(num value, int from, int to) =>
    value.truncateToDouble() == value && value >= from && value <= to;

/// Number of visible fraction digits (`v` in CLDR) of [value].
///
/// Integral values have none. Other values are measured from their shortest
/// decimal representation, which never ends in `0`; exponential notation
/// reports `0`, since its digits are not part of the shortest form.
int _visibleFractionDigits(num value) {
  if (value == value.truncateToDouble()) return 0;
  final text = value.abs().toString();
  if (text.contains('e')) return 0;
  final dot = text.indexOf('.');
  return dot < 0 ? 0 : text.length - dot - 1;
}

/// The visible fraction digits (`f` in CLDR) of [value] as an integer.
///
/// `1.25` gives `25` and `0.5` gives `5`; integral values and exponential
/// notation give `0`.
int _fractionDigits(num value) {
  if (value == value.truncateToDouble()) return 0;
  final text = value.abs().toString();
  if (text.contains('e')) return 0;
  final dot = text.indexOf('.');
  if (dot < 0) return 0;
  return int.tryParse(text.substring(dot + 1)) ?? 0;
}
