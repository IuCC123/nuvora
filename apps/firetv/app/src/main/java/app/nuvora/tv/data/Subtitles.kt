package app.nuvora.tv.data

import kotlinx.serialization.Serializable

// Choosing subtitles the way the web does (packages/core/src/subtitles.ts):
// the viewer's last choice for this show or movie, else the account setting.

/** How subtitles look on this TV. */
@Serializable
data class SubtitleStyle(val size: String = "medium", val background: String = "shadow") {
    companion object {
        val SIZES = listOf("small" to "Small", "medium" to "Medium", "large" to "Large")
        val BACKGROUNDS = listOf("none" to "None", "shadow" to "Shadow", "box" to "Box")
    }
}

/**
 * The viewer's last choice for a show or movie. A track is remembered by what
 * it is rather than its position, so the choice carries to the next episode
 * even when that file orders its tracks differently.
 */
@Serializable
data class RememberedSubtitle(
    val off: Boolean = false,
    val language: String? = null,
    val title: String? = null,
    val forced: Boolean = false,
    val hearingImpaired: Boolean = false,
    val styled: Boolean = false,
)

/** A subtitle track the player can show, as the chooser and the menu see it. */
data class SubtitleOption(
    /** The server's three letters, or null when unknown. */
    val language: String?,
    val title: String?,
    val forced: Boolean,
    val isDefault: Boolean,
    val hearingImpaired: Boolean,
    val styled: Boolean,
) {
    fun remembered() = RememberedSubtitle(false, language, title, forced, hearingImpaired, styled)

    /** `"Spanish · Signs & Songs · Forced"`. */
    val label: String
        get() {
            val name = languageName(language)
            val tags = buildList {
                if (forced) add("Forced")
                if (hearingImpaired) add("SDH")
                if (styled) add("Styled")
            }
            val t = title?.trim()?.takeIf { it.isNotEmpty() }
            val redundant = t == null || t.equals(name, ignoreCase = true) || tags.any { it.equals(t, ignoreCase = true) }
            return (listOf(name) + listOfNotNull(t.takeUnless { redundant }) + tags).joinToString(" · ")
        }
}

/** The option to start with (its index), or null for none. */
fun chooseSubtitle(
    options: List<SubtitleOption>,
    audioLanguage: String?,
    mode: String,
    preferredLanguage: String?,
    remembered: RememberedSubtitle?,
): Int? {
    if (remembered?.off == true) return null
    if (remembered != null) closest(options, remembered)?.let { return it }

    val preferred = languageCode(preferredLanguage)
    val inPreferred = options.indices.filter { preferred != null && options[it].language == preferred }
    val flaggedDefault = options.indices.firstOrNull { options[it].isDefault }
    return when (mode) {
        "off" -> null
        "always" -> best(options, inPreferred) ?: flaggedDefault
        else -> {
            val audio = languageCode(audioLanguage)
            // Unknown audio counts as the viewer's language: subtitles don't appear over a film they may understand.
            val foreignAudio = audio != null && audio != preferred
            if (foreignAudio && inPreferred.isNotEmpty()) best(options, inPreferred)
            else best(options, inPreferred.filter { options[it].forced }) ?: flaggedDefault
        }
    }
}

/** Non-forced first, then the file's default flag, then not SDH, then file order. */
private fun best(options: List<SubtitleOption>, among: List<Int>): Int? {
    fun rank(o: SubtitleOption) = (if (o.forced) 4 else 0) + (if (o.isDefault) 0 else 2) + (if (o.hearingImpaired) 1 else 0)
    return among.minByOrNull { rank(options[it]) }
}

/** The same-language option most like the remembered one. */
private fun closest(options: List<SubtitleOption>, want: RememberedSubtitle): Int? {
    fun score(o: SubtitleOption) =
        (if (o.title == want.title) 8 else 0) +
            (if (o.forced == want.forced) 4 else 0) +
            (if (o.hearingImpaired == want.hearingImpaired) 2 else 0) +
            (if (o.styled == want.styled) 1 else 0)
    return options.indices.filter { options[it].language == want.language }.maxByOrNull { score(options[it]) }
}
