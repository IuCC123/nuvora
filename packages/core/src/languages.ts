/**
 * Every ISO 639-1 language with its ISO 639-2 code and English name, so the
 * TV apps can name and match languages without `Intl`. One row per language:
 * `"<639-1> <639-2/B>[/<639-2/T>] <English name>"`. Matroska and ffprobe use
 * the bibliographic code (`fre`, `ger`, `chi`); MP4 files often carry the
 * terminology one (`fra`, `deu`, `zho`), so both are recognised.
 */
const TABLE = `
aa aar Afar
ab abk Abkhazian
ae ave Avestan
af afr Afrikaans
ak aka Akan
am amh Amharic
an arg Aragonese
ar ara Arabic
as asm Assamese
av ava Avaric
ay aym Aymara
az aze Azerbaijani
ba bak Bashkir
be bel Belarusian
bg bul Bulgarian
bi bis Bislama
bm bam Bambara
bn ben Bengali
bo tib/bod Tibetan
br bre Breton
bs bos Bosnian
ca cat Catalan
ce che Chechen
ch cha Chamorro
co cos Corsican
cr cre Cree
cs cze/ces Czech
cu chu Church Slavic
cv chv Chuvash
cy wel/cym Welsh
da dan Danish
de ger/deu German
dv div Divehi
dz dzo Dzongkha
ee ewe Ewe
el gre/ell Greek
en eng English
eo epo Esperanto
es spa Spanish
et est Estonian
eu baq/eus Basque
fa per/fas Persian
ff ful Fulah
fi fin Finnish
fj fij Fijian
fo fao Faroese
fr fre/fra French
fy fry Western Frisian
ga gle Irish
gd gla Scottish Gaelic
gl glg Galician
gn grn Guarani
gu guj Gujarati
gv glv Manx
ha hau Hausa
he heb Hebrew
hi hin Hindi
ho hmo Hiri Motu
hr hrv Croatian
ht hat Haitian Creole
hu hun Hungarian
hy arm/hye Armenian
hz her Herero
ia ina Interlingua
id ind Indonesian
ie ile Interlingue
ig ibo Igbo
ii iii Sichuan Yi
ik ipk Inupiaq
io ido Ido
is ice/isl Icelandic
it ita Italian
iu iku Inuktitut
ja jpn Japanese
jv jav Javanese
ka geo/kat Georgian
kg kon Kongo
ki kik Kikuyu
kj kua Kuanyama
kk kaz Kazakh
kl kal Kalaallisut
km khm Khmer
kn kan Kannada
ko kor Korean
kr kau Kanuri
ks kas Kashmiri
ku kur Kurdish
kv kom Komi
kw cor Cornish
ky kir Kyrgyz
la lat Latin
lb ltz Luxembourgish
lg lug Ganda
li lim Limburgish
ln lin Lingala
lo lao Lao
lt lit Lithuanian
lu lub Luba-Katanga
lv lav Latvian
mg mlg Malagasy
mh mah Marshallese
mi mao/mri Maori
mk mac/mkd Macedonian
ml mal Malayalam
mn mon Mongolian
mr mar Marathi
ms may/msa Malay
mt mlt Maltese
my bur/mya Burmese
na nau Nauru
nb nob Norwegian Bokmål
nd nde North Ndebele
ne nep Nepali
ng ndo Ndonga
nl dut/nld Dutch
nn nno Norwegian Nynorsk
no nor Norwegian
nr nbl South Ndebele
nv nav Navajo
ny nya Chichewa
oc oci Occitan
oj oji Ojibwa
om orm Oromo
or ori Odia
os oss Ossetic
pa pan Punjabi
pi pli Pali
pl pol Polish
ps pus Pashto
pt por Portuguese
qu que Quechua
rm roh Romansh
rn run Rundi
ro rum/ron Romanian
ru rus Russian
rw kin Kinyarwanda
sa san Sanskrit
sc srd Sardinian
sd snd Sindhi
se sme Northern Sami
sg sag Sango
si sin Sinhala
sk slo/slk Slovak
sl slv Slovenian
sm smo Samoan
sn sna Shona
so som Somali
sq alb/sqi Albanian
sr srp Serbian
ss ssw Swati
st sot Southern Sotho
su sun Sundanese
sv swe Swedish
sw swa Swahili
ta tam Tamil
te tel Telugu
tg tgk Tajik
th tha Thai
ti tir Tigrinya
tk tuk Turkmen
tl tgl Tagalog
tn tsn Tswana
to ton Tongan
tr tur Turkish
ts tso Tsonga
tt tat Tatar
tw twi Twi
ty tah Tahitian
ug uig Uyghur
uk ukr Ukrainian
ur urd Urdu
uz uzb Uzbek
ve ven Venda
vi vie Vietnamese
vo vol Volapük
wa wln Walloon
wo wol Wolof
xh xho Xhosa
yi yid Yiddish
yo yor Yoruba
za zha Zhuang
zh chi/zho Chinese
zu zul Zulu
`;

export interface Language {
  /** ISO 639-2/B, the code the server stores and the preference uses: `"eng"`, `"fre"`. */
  code: string;
  /** ISO 639-1: `"en"`, `"fr"`. */
  short: string;
  /** In English: `"French"`. */
  name: string;
}

const byCode = new Map<string, Language>();
/** Every language, by English name. */
export const LANGUAGES: readonly Language[] = TABLE.trim()
  .split("\n")
  .map((row) => {
    const [short = "", codes = "", ...name] = row.split(" ");
    const [code = "", terminology] = codes.split("/");
    const language = { code, short, name: name.join(" ") };
    for (const key of [short, code, terminology]) if (key) byCode.set(key, language);
    return language;
  })
  .sort((a, b) => a.name.localeCompare(b.name));

/**
 * A language from any code a file or device carries: ISO 639-1 (`"pt"`),
 * either ISO 639-2 form (`"fre"`, `"fra"`) or a BCP 47 tag (`"pt-BR"`, `"zh_Hant"`).
 * Undefined for `"und"`, `"mul"`, empty and unknown codes.
 */
export function findLanguage(code: string | null | undefined): Language | undefined {
  if (!code) return undefined;
  const primary = code.trim().toLowerCase().split(/[-_]/)[0] ?? "";
  return byCode.get(primary);
}

/** The ISO 639-2/B code for any language code, so `"fra"`, `"fr-CA"` and `"fre"` compare equal. Null when unknown. */
export function languageCode(code: string | null | undefined): string | null {
  return findLanguage(code)?.code ?? null;
}

/** `"jpn"` -> `"Japanese"`. An unknown code comes back as it was; a missing one reads "Unknown". */
export function languageName(code: string | null | undefined): string {
  if (!code || code === "und") return "Unknown";
  return findLanguage(code)?.name ?? code;
}
