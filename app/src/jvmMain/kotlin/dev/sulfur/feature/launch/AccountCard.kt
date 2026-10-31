package dev.sulfur.feature.launch

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import dev.sulfur.data.LocalImages
import dev.sulfur.data.SulfurRepository
import dev.sulfur.data.UiMessage
import dev.sulfur.data.rememberRemoteImage
import dev.sulfur.ffi.AccountInfo
import dev.sulfur.ffi.AccountKind
import dev.sulfur.ffi.SignInSnapshot
import dev.sulfur.ffi.SignInStage
import dev.sulfur.ffi.SignInTask
import dev.sulfur.resources.Res
import dev.sulfur.resources.account_add_microsoft
import dev.sulfur.resources.account_edit_name
import dev.sulfur.resources.account_microsoft
import dev.sulfur.resources.account_ms_unavailable
import dev.sulfur.resources.account_need_microsoft
import dev.sulfur.resources.account_offline
import dev.sulfur.resources.account_sign_in
import dev.sulfur.resources.account_sign_out
import dev.sulfur.resources.account_switch
import dev.sulfur.resources.action_cancel
import dev.sulfur.resources.action_retry
import dev.sulfur.resources.action_save
import dev.sulfur.resources.ic_add
import dev.sulfur.resources.ic_arrow_drop_down
import dev.sulfur.resources.ic_check
import dev.sulfur.resources.ic_delete
import dev.sulfur.resources.ic_edit
import dev.sulfur.resources.ic_open_in_new
import dev.sulfur.resources.ic_person
import dev.sulfur.resources.launch_offline_hint
import dev.sulfur.resources.launch_offline_name
import dev.sulfur.resources.msg_signed_in
import dev.sulfur.resources.signin_copy_open
import dev.sulfur.resources.signin_failed
import dev.sulfur.resources.signin_instructions
import dev.sulfur.resources.signin_minecraft
import dev.sulfur.resources.signin_profile
import dev.sulfur.resources.signin_starting
import dev.sulfur.resources.signin_waiting
import dev.sulfur.resources.signin_xbox
import dev.sulfur.util.openUrl
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource

/**
 * Who plays: the current account with a menu to switch, add or sign out of
 * accounts. Without any usable account it offers Microsoft sign-in.
 */
@Composable
internal fun AccountCard(repository: SulfurRepository) {
    val account by repository.currentAccount.collectAsState()
    val accounts by repository.accounts.collectAsState()
    val policy by repository.accountPolicy.collectAsState()
    var signingIn by remember { mutableStateOf(false) }
    var editingName by remember { mutableStateOf(false) }
    var menuOpen by remember { mutableStateOf(false) }

    Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLow, modifier = Modifier.fillMaxWidth()) {
        val current = account
        if (current == null) {
            // Outside mainland China nothing can be played without a Microsoft account.
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    stringResource(if (policy.microsoftAvailable) Res.string.account_need_microsoft else Res.string.account_ms_unavailable),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Button(onClick = { signingIn = true }, enabled = policy.microsoftAvailable, shapes = ButtonDefaults.shapes()) {
                    Icon(painterResource(Res.drawable.ic_person), null, Modifier.size(ButtonDefaults.IconSize))
                    Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                    Text(stringResource(Res.string.account_sign_in))
                }
            }
            return@Surface
        }
        Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
            AccountAvatar(current, 48.dp)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(current.name, style = MaterialTheme.typography.titleMediumEmphasized, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    stringResource(if (current.kind == AccountKind.MICROSOFT) Res.string.account_microsoft else Res.string.account_offline),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (current.kind == AccountKind.OFFLINE) {
                IconButton(onClick = { editingName = true }) {
                    Icon(painterResource(Res.drawable.ic_edit), stringResource(Res.string.account_edit_name), Modifier.size(20.dp))
                }
            }
            Box {
                IconButton(onClick = { menuOpen = true }) {
                    Icon(painterResource(Res.drawable.ic_arrow_drop_down), stringResource(Res.string.account_switch))
                }
                DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                    val choices = accounts + listOfNotNull(
                        AccountInfo("", AccountKind.OFFLINE, repository.settings.value.offlineName, null).takeIf { policy.offlineAllowed }
                    )
                    choices.forEach { choice ->
                        DropdownMenuItem(
                            text = { Text(choice.name) },
                            leadingIcon = { AccountAvatar(choice, 28.dp) },
                            trailingIcon = if (choice.id == current.id && choice.kind == current.kind) {
                                { Icon(painterResource(Res.drawable.ic_check), null) }
                            } else null,
                            onClick = {
                                repository.selectAccount(choice)
                                menuOpen = false
                            },
                        )
                    }
                    HorizontalDivider()
                    DropdownMenuItem(
                        text = { Text(stringResource(Res.string.account_add_microsoft)) },
                        leadingIcon = { Icon(painterResource(Res.drawable.ic_add), null) },
                        enabled = policy.microsoftAvailable,
                        onClick = {
                            menuOpen = false
                            signingIn = true
                        },
                    )
                    if (current.kind == AccountKind.MICROSOFT) {
                        DropdownMenuItem(
                            text = { Text(stringResource(Res.string.account_sign_out, current.name)) },
                            leadingIcon = { Icon(painterResource(Res.drawable.ic_delete), null) },
                            onClick = {
                                menuOpen = false
                                repository.removeAccount(current)
                            },
                        )
                    }
                }
            }
        }
    }
    if (signingIn) {
        SignInDialog(repository, onDismiss = { signingIn = false })
    }
    if (editingName) {
        OfflineNameDialog(repository, onDismiss = { editingName = false })
    }
}

/** The player's face from their skin, or the first letter of their name. */
@Composable
internal fun AccountAvatar(account: AccountInfo, size: Dp) {
    val skin by rememberRemoteImage(account.skinUrl, LocalImages.current)
    val shape = MaterialShapes.Cookie9Sided.toShape()
    val face = skin
    if (face != null) {
        Canvas(Modifier.size(size).clip(MaterialTheme.shapes.small)) {
            val dst = IntSize(this.size.width.toInt(), this.size.height.toInt())
            // The face and the hat layer over it, 8×8 pixels each on a 64-pixel-wide skin.
            val scale = face.width / 64
            for (x in listOf(8, 40)) {
                drawImage(
                    face,
                    srcOffset = IntOffset(x * scale, 8 * scale),
                    srcSize = IntSize(8 * scale, 8 * scale),
                    dstSize = dst,
                    filterQuality = FilterQuality.None,
                )
            }
        }
        return
    }
    Box(
        Modifier.size(size).clip(shape).background(MaterialTheme.colorScheme.tertiaryContainer),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            account.name.take(1).uppercase(),
            style = if (size >= 40.dp) MaterialTheme.typography.titleLargeEmphasized else MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onTertiaryContainer,
        )
    }
}

@Composable
private fun OfflineNameDialog(repository: SulfurRepository, onDismiss: () -> Unit) {
    var name by remember { mutableStateOf(repository.settings.value.offlineName) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(Res.string.account_edit_name)) },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = { name = it.take(16) },
                label = { Text(stringResource(Res.string.launch_offline_name)) },
                supportingText = { Text(stringResource(Res.string.launch_offline_hint)) },
                singleLine = true,
            )
        },
        confirmButton = {
            TextButton(
                onClick = {
                    repository.updateSettings { it.copy(offlineName = name.trim().ifEmpty { "Player" }) }
                    onDismiss()
                },
            ) { Text(stringResource(Res.string.action_save)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(Res.string.action_cancel)) } },
    )
}

/**
 * Microsoft sign-in with a device code: the player enters the code shown here
 * on Microsoft's page, and Sulfur finishes the rest.
 */
@Composable
private fun SignInDialog(repository: SulfurRepository, onDismiss: () -> Unit) {
    var attempt by remember { mutableStateOf(0) }
    var task by remember { mutableStateOf<SignInTask?>(null) }
    var snapshot by remember { mutableStateOf<SignInSnapshot?>(null) }
    LaunchedEffect(attempt) {
        val started = repository.signInMicrosoft()
        task = started
        try {
            while (true) {
                val now = started.snapshot()
                snapshot = now
                if (now.stage == SignInStage.DONE) {
                    repository.accountsChanged()
                    now.account?.let {
                        repository.selectAccount(it)
                        repository.notify(UiMessage(Res.string.msg_signed_in, it.name))
                    }
                    onDismiss()
                    break
                }
                if (now.stage == SignInStage.FAILED || now.stage == SignInStage.CANCELLED) break
                delay(300)
            }
        } finally {
            // Leaving the dialog (or retrying) stops a sign-in still waiting for the player.
            started.cancel()
        }
    }
    val close = {
        task?.cancel()
        onDismiss()
    }
    val stage = snapshot?.stage ?: SignInStage.STARTING
    AlertDialog(
        onDismissRequest = close,
        icon = { Icon(painterResource(Res.drawable.ic_person), null) },
        title = { Text(stringResource(if (stage == SignInStage.FAILED) Res.string.signin_failed else Res.string.account_sign_in)) },
        text = {
            Column(Modifier.widthIn(min = 320.dp, max = 440.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                when (stage) {
                    SignInStage.WAITING_FOR_PLAYER -> DeviceCode(snapshot!!)
                    SignInStage.FAILED -> Text(
                        snapshot?.error.orEmpty(),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                    else -> Progress(
                        stringResource(
                            when (stage) {
                                SignInStage.XBOX -> Res.string.signin_xbox
                                SignInStage.MINECRAFT -> Res.string.signin_minecraft
                                SignInStage.PROFILE -> Res.string.signin_profile
                                else -> Res.string.signin_starting
                            }
                        )
                    )
                }
            }
        },
        confirmButton = {
            if (stage == SignInStage.FAILED) {
                TextButton(onClick = { attempt++ }) { Text(stringResource(Res.string.action_retry)) }
            }
        },
        dismissButton = { TextButton(onClick = close) { Text(stringResource(Res.string.action_cancel)) } },
    )
}

@Composable
private fun DeviceCode(snapshot: SignInSnapshot) {
    val code = snapshot.userCode.orEmpty()
    val uri = snapshot.verificationUri.orEmpty()
    val clipboard = LocalClipboardManager.current
    val scope = rememberCoroutineScope()
    Text(stringResource(Res.string.signin_instructions, uri), style = MaterialTheme.typography.bodyMedium)
    Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.primaryContainer, modifier = Modifier.fillMaxWidth()) {
        SelectionContainer {
            Text(
                code,
                style = MaterialTheme.typography.displaySmall.copy(fontFamily = FontFamily.Monospace),
                color = MaterialTheme.colorScheme.onPrimaryContainer,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(vertical = 16.dp).fillMaxWidth(),
            )
        }
    }
    FilledTonalButton(
        onClick = {
            clipboard.setText(AnnotatedString(code))
            scope.launch { openUrl(uri) }
        },
        shapes = ButtonDefaults.shapes(),
    ) {
        Icon(painterResource(Res.drawable.ic_open_in_new), null, Modifier.size(ButtonDefaults.IconSize))
        Spacer(Modifier.width(ButtonDefaults.IconSpacing))
        Text(stringResource(Res.string.signin_copy_open))
    }
    Progress(stringResource(Res.string.signin_waiting))
}

@Composable
private fun Progress(text: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        LoadingIndicator(Modifier.size(36.dp))
        Spacer(Modifier.width(12.dp))
        Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}
