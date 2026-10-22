package app.nuvora.tv.ui.together

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import androidx.navigation.NavHostController
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import app.nuvora.tv.data.ApiException
import app.nuvora.tv.data.NuvoraApi
import app.nuvora.tv.data.PartyInvite
import app.nuvora.tv.data.PartyLink
import app.nuvora.tv.data.PartyPerson
import app.nuvora.tv.data.PartyView
import app.nuvora.tv.data.positionAt
import app.nuvora.tv.store
import app.nuvora.tv.ui.PlayerRoute
import app.nuvora.tv.ui.TogetherRoute
import app.nuvora.tv.ui.common.Load
import app.nuvora.tv.ui.common.LocalApi
import app.nuvora.tv.ui.common.NvButton
import app.nuvora.tv.ui.common.NvIcons
import app.nuvora.tv.ui.common.initialFocus
import app.nuvora.tv.ui.common.rememberInitialFocus
import app.nuvora.tv.ui.common.rememberLoad
import app.nuvora.tv.ui.home.Section
import app.nuvora.tv.ui.home.Shell
import app.nuvora.tv.ui.theme.Nv
import kotlinx.coroutines.launch

/** Opens the player on what the party is playing, at the party's position. */
fun NavHostController.followParty(link: PartyLink, party: PartyView) {
    val playback = party.playback ?: return
    navigate(PlayerRoute(playback.itemId, playback.fileId, playback.positionAt(link.serverNow()).coerceAtLeast(0))) {
        // Replace a player that's open, so Back still leaves the player.
        popUpTo<PlayerRoute> { inclusive = true }
    }
}

/** Joins a party; then to what it's watching, or to its page when nothing is on yet. */
suspend fun join(nav: NavHostController, api: NuvoraApi, link: PartyLink, invite: PartyInvite): String? = try {
    val party = api.joinParty(invite.partyId)
    link.setParty(party)
    link.setInvites(link.state.value.invites.filter { it.partyId != invite.partyId })
    if (party.playback != null) nav.followParty(link, party) else nav.navigate(TogetherRoute) { launchSingleTop = true }
    null
} catch (e: ApiException) {
    link.setInvites(link.state.value.invites.filter { it.partyId != invite.partyId })
    if (e.status == 404) "That invitation has ended." else e.message
}

suspend fun decline(api: NuvoraApi, link: PartyLink, invite: PartyInvite) {
    runCatching { api.declineParty(invite.partyId) }
    link.setInvites(link.state.value.invites.filter { it.partyId != invite.partyId })
}

/**
 * Watch Together: invitations to answer, who's in the group and what it's
 * watching, and who else can be invited.
 */
@Composable
fun TogetherScreen(nav: NavHostController, me: Long?) {
    val api = LocalApi.current
    val link = LocalContext.current.store.party
    val snapshot by link.state.collectAsState()
    val party = snapshot.party
    val scope = rememberCoroutineScope()
    val (people) = rememberLoad { people() }
    var problem by remember { mutableStateOf<String?>(null) }
    val focus = rememberInitialFocus(party?.id)

    fun act(block: suspend () -> Unit) {
        scope.launch {
            problem = null
            try {
                block()
            } catch (e: ApiException) {
                problem = e.message
            }
        }
    }

    Shell(nav, Section.Together) {
        Column(Modifier.fillMaxSize().padding(start = 48.dp, end = 48.dp, top = 36.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Watch together", style = MaterialTheme.typography.headlineMedium, color = Nv.textPrimary)
                if (!snapshot.connected) {
                    Spacer(Modifier.width(16.dp))
                    Text("Reconnecting…", style = MaterialTheme.typography.titleSmall, color = Nv.textMuted)
                }
            }
            Spacer(Modifier.height(8.dp))
            Text(
                if (party == null) "Invite someone, then press Play on anything. It starts for everyone at the same moment, and pausing or skipping stays in sync."
                else "Press Play on any movie or episode and it starts for everyone in the group.",
                style = MaterialTheme.typography.bodyLarge,
                color = Nv.textSecondary,
                modifier = Modifier.width(820.dp),
            )
            problem?.let {
                Spacer(Modifier.height(8.dp))
                Text(it, style = MaterialTheme.typography.titleSmall, color = Nv.danger)
            }
            Spacer(Modifier.height(28.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(56.dp), modifier = Modifier.fillMaxSize()) {
                LazyColumn(
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                    contentPadding = PaddingValues(bottom = 48.dp),
                    modifier = Modifier.weight(1f).focusRestorer(),
                ) {
                    if (snapshot.invites.isNotEmpty()) {
                        item { Heading("Invitations") }
                        items(snapshot.invites, key = { it.key }) { invite ->
                            InviteCard(
                                invite,
                                onJoin = { act { join(nav, api, link, invite)?.let { problem = it } } },
                                onDecline = { act { decline(api, link, invite) } },
                                modifier = if (invite == snapshot.invites.first()) Modifier.initialFocus(focus) else Modifier,
                            )
                        }
                        item { Spacer(Modifier.height(18.dp)) }
                    }
                    if (party != null) {
                        item { NowPlaying(party, onWatch = { nav.followParty(link, party) }, onFocusFirst = if (snapshot.invites.isEmpty()) focus else null) }
                        item { Heading("In this group") }
                        items(party.members, key = { "m${it.id}" }) { member ->
                            PersonLine(
                                member.displayName,
                                you = member.id == me,
                                status = when {
                                    member.id == party.hostId -> "Host"
                                    member.online -> "Here"
                                    else -> "Away"
                                },
                                away = !member.online,
                            )
                        }
                        items(party.invited, key = { "i${it.id}" }) { person ->
                            PersonButton(person, "Invited · cancel", on = true) {
                                act { link.setParty(api.cancelInvite(person.id)) }
                            }
                        }
                        item {
                            Spacer(Modifier.height(14.dp))
                            val nothingAbove = snapshot.invites.isEmpty() && party.playback == null && party.invited.isEmpty()
                            NvButton(
                                "Leave group",
                                { act { api.leaveParty(); link.setParty(null) } },
                                if (nothingAbove) Modifier.initialFocus(focus) else Modifier,
                                icon = NvIcons.SignOut,
                            )
                        }
                    }
                }
                LazyColumn(
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                    contentPadding = PaddingValues(bottom = 48.dp),
                    modifier = Modifier.weight(1f).focusRestorer(),
                ) {
                    item { Heading(if (party != null) "Invite more" else "Invite") }
                    when (val state = people) {
                        Load.Loading -> item { Hint("Loading…") }
                        is Load.Failed -> item { Hint("Couldn't load who's on this server.") }
                        is Load.Ready -> {
                            val inGroup = party?.let { p -> p.members.map { it.id } + p.invited.map { it.id } }.orEmpty().toSet()
                            val invitable = state.value.filter { it.id !in inGroup }
                            if (invitable.isEmpty()) {
                                item {
                                    Hint(if (state.value.isNotEmpty()) "Everyone's in already." else "No one else has an account yet. An admin can add people in Settings › Users, in a browser.")
                                }
                            }
                            items(invitable, key = { it.id }) { person ->
                                val first = party == null && snapshot.invites.isEmpty() && person == invitable.first()
                                PersonButton(person, "Invite", on = false, modifier = if (first) Modifier.initialFocus(focus) else Modifier) {
                                    act { link.setParty(api.invite(person.id)) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun Heading(text: String) {
    Text(text, style = MaterialTheme.typography.titleLarge, color = Nv.textPrimary, modifier = Modifier.padding(bottom = 4.dp))
}

@Composable
private fun Hint(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = Nv.textMuted)
}

/** A round badge with someone's initials. */
@Composable
fun Avatar(name: String, modifier: Modifier = Modifier, size: Int = 44) {
    Box(modifier.size(size.dp).clip(CircleShape).background(Nv.hairline), contentAlignment = Alignment.Center) {
        Text(initials(name), style = MaterialTheme.typography.labelLarge, color = Nv.textPrimary)
    }
}

fun initials(name: String) = name.split(" ").filter { it.isNotBlank() }.take(2).joinToString("") { it.first().uppercase() }.ifEmpty { "?" }

@Composable
private fun PersonLine(name: String, you: Boolean, status: String, away: Boolean) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        Avatar(name, Modifier.alpha(if (away) 0.5f else 1f))
        Spacer(Modifier.width(16.dp))
        Text(if (you) "$name (you)" else name, style = MaterialTheme.typography.titleMedium, color = Nv.textPrimary, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(status, style = MaterialTheme.typography.labelLarge, color = if (status == "Away") Nv.textMuted else Nv.textSecondary)
    }
}

@Composable
private fun PersonButton(person: PartyPerson, action: String, on: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Surface(
        onClick = onClick,
        shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(12.dp)),
        colors = ClickableSurfaceDefaults.colors(
            containerColor = Nv.surface,
            contentColor = Nv.textPrimary,
            focusedContainerColor = Nv.textPrimary,
            focusedContentColor = Nv.canvas,
        ),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1.02f),
        modifier = modifier.fillMaxWidth(),
    ) {
        Row(Modifier.padding(horizontal = 16.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            Avatar(person.displayName)
            Spacer(Modifier.width(16.dp))
            Text(person.displayName, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (on) Icon(NvIcons.Check, null, modifier = Modifier.size(18.dp).padding(end = 2.dp))
            Spacer(Modifier.width(8.dp))
            Text(action, style = MaterialTheme.typography.labelLarge)
        }
    }
}

/** What the group is watching, with a way back to it. */
@Composable
private fun NowPlaying(party: PartyView, onWatch: () -> Unit, onFocusFirst: androidx.compose.ui.focus.FocusRequester?) {
    val playback = party.playback
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(Nv.surface).padding(20.dp),
    ) {
        if (playback == null) {
            Text("NOTHING PLAYING YET", style = MaterialTheme.typography.labelSmall, color = Nv.accent)
            Spacer(Modifier.height(6.dp))
            Text("Press Play on any movie or episode and everyone starts together.", style = MaterialTheme.typography.bodyLarge, color = Nv.textSecondary)
        } else {
            Text(if (playback.playing) "PLAYING NOW" else "PAUSED", style = MaterialTheme.typography.labelSmall, color = Nv.accent)
            Spacer(Modifier.height(6.dp))
            Text(playback.title, style = MaterialTheme.typography.titleLarge, color = Nv.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            playback.subtitle?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = Nv.textSecondary) }
            Spacer(Modifier.height(14.dp))
            NvButton("Watch", onWatch, if (onFocusFirst != null) Modifier.initialFocus(onFocusFirst) else Modifier, icon = NvIcons.Play, primary = true)
        }
    }
    Spacer(Modifier.height(18.dp))
}

@Composable
private fun InviteCard(invite: PartyInvite, onJoin: () -> Unit, onDecline: () -> Unit, modifier: Modifier = Modifier) {
    val with = invite.members.filter { it.id != invite.from.id }.map { it.displayName }
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(Nv.surface).padding(18.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Avatar(invite.from.displayName, size = 52)
        Spacer(Modifier.width(18.dp))
        Column(Modifier.weight(1f)) {
            Text(
                "${invite.from.displayName} wants to watch together" + if (with.isNotEmpty()) " with ${with.joinToString(", ")}" else "",
                style = MaterialTheme.typography.titleMedium,
                color = Nv.textPrimary,
            )
            Spacer(Modifier.height(12.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                NvButton("Join", onJoin, modifier, icon = NvIcons.Users, primary = true)
                NvButton("Not now", onDecline)
            }
        }
    }
}

/**
 * A new invitation, wherever the person is in the app: a card in the corner
 * that takes focus, so OK joins. Back leaves it for later, on the Watch
 * together page.
 */
@Composable
fun InviteDialog(nav: NavHostController, invite: PartyInvite, onClose: () -> Unit) {
    val api = LocalApi.current
    val link = LocalContext.current.store.party
    val scope = rememberCoroutineScope()
    var problem by remember { mutableStateOf<String?>(null) }
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        (LocalView.current.parent as? DialogWindowProvider)?.window?.setDimAmount(0f)
        val focus = rememberInitialFocus()
        Box(Modifier.fillMaxSize().background(Brush.horizontalGradient(0.4f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.6f)))) {
        Column(
            Modifier.align(Alignment.TopEnd).padding(40.dp).widthIn(min = 420.dp, max = 560.dp).width(IntrinsicSize.Max)
                .clip(RoundedCornerShape(16.dp)).background(Nv.surface)
                .border(1.dp, Nv.hairline, RoundedCornerShape(16.dp)).padding(24.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Avatar(invite.from.displayName, size = 52)
                Spacer(Modifier.width(18.dp))
                Column {
                    Text("WATCH TOGETHER", style = MaterialTheme.typography.labelSmall, color = Nv.accent)
                    Spacer(Modifier.height(4.dp))
                    val with = invite.members.filter { it.id != invite.from.id }.map { it.displayName }
                    Text(
                        "${invite.from.displayName} invited you" + if (with.isNotEmpty()) ", with ${with.joinToString(", ")}" else "",
                        style = MaterialTheme.typography.titleMedium,
                        color = Nv.textPrimary,
                    )
                }
            }
            problem?.let {
                Spacer(Modifier.height(10.dp))
                Text(it, style = MaterialTheme.typography.bodyMedium, color = Nv.danger)
            }
            Spacer(Modifier.height(20.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                NvButton(
                    "Join",
                    {
                        scope.launch {
                            val failed = join(nav, api, link, invite)
                            if (failed == null) onClose() else problem = failed
                        }
                    },
                    Modifier.initialFocus(focus),
                    icon = NvIcons.Users,
                    primary = true,
                )
                NvButton("Not now", { scope.launch { decline(api, link, invite); onClose() } })
                NvButton("Later", onClose)
            }
        }
        }
    }
}
