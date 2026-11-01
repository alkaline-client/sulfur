package dev.sulfur.feature.settings

import dev.sulfur.ui.components.SelectField
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextFieldLabelPosition
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import dev.sulfur.ffi.GarbageCollector
import dev.sulfur.ffi.JavaRuntime
import dev.sulfur.resources.Res
import dev.sulfur.resources.gc_auto
import dev.sulfur.resources.gc_auto_hint
import dev.sulfur.resources.gc_g1_hint
import dev.sulfur.resources.gc_parallel_hint
import dev.sulfur.resources.gc_shenandoah_hint
import dev.sulfur.resources.gc_zgc_hint
import dev.sulfur.resources.ic_coffee
import dev.sulfur.resources.version_global
import dev.sulfur.resources.settings_env
import dev.sulfur.resources.settings_env_hint
import dev.sulfur.resources.settings_gc
import dev.sulfur.resources.settings_java
import dev.sulfur.resources.settings_java_downloaded
import dev.sulfur.resources.settings_pre_launch
import dev.sulfur.resources.settings_pre_launch_hint
import dev.sulfur.resources.settings_wrapper
import dev.sulfur.resources.settings_wrapper_hint
import dev.sulfur.ui.components.ConnectedChoice
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource

// Fields shared by Settings → Java (global values) and a version's settings
// (overrides, where null or blank follows the global value).

/** Product names; only "JVM default" is translated. */
@Composable
internal fun gcLabel(gc: GarbageCollector): String = when (gc) {
    GarbageCollector.AUTO -> stringResource(Res.string.gc_auto)
    GarbageCollector.G1 -> "G1"
    GarbageCollector.ZGC -> "ZGC"
    GarbageCollector.SHENANDOAH -> "Shenandoah"
    GarbageCollector.PARALLEL -> "Parallel"
}

@Composable
private fun gcHint(gc: GarbageCollector): String = stringResource(
    when (gc) {
        GarbageCollector.AUTO -> Res.string.gc_auto_hint
        GarbageCollector.G1 -> Res.string.gc_g1_hint
        GarbageCollector.ZGC -> Res.string.gc_zgc_hint
        GarbageCollector.SHENANDOAH -> Res.string.gc_shenandoah_hint
        GarbageCollector.PARALLEL -> Res.string.gc_parallel_hint
    }
)

/**
 * The garbage collector. With [inherited] (the global choice) a "Default"
 * option leads, standing for null.
 */
@Composable
internal fun GcChoice(selected: GarbageCollector?, inherited: GarbageCollector?, onSelect: (GarbageCollector?) -> Unit) {
    Label(stringResource(Res.string.settings_gc))
    val options: List<GarbageCollector?> =
        (if (inherited != null) listOf<GarbageCollector?>(null) else emptyList()) + GarbageCollector.entries
    ConnectedChoice(
        options = options,
        selected = selected,
        label = { gc -> gc?.let { gcLabel(it) } ?: stringResource(Res.string.version_global) },
        onSelect = onSelect,
        fill = false,
    )
    Hint(gcHint(selected ?: inherited ?: GarbageCollector.AUTO))
}

/** A text setting; [placeholder] shows the inherited value when the field is empty. */
@Composable
internal fun OptionField(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    hint: String? = null,
    placeholder: String? = null,
    singleLine: Boolean = true,
    monospace: Boolean = false,
) {
    SettingTextField(
        value = value,
        onChange = onChange,
        label = label,
        placeholder = placeholder,
        hint = hint,
        lineLimits = if (singleLine) TextFieldLineLimits.SingleLine else TextFieldLineLimits.MultiLine(3, 8),
        textStyle = if (monospace) MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace)
        else MaterialTheme.typography.bodyLarge,
        modifier = Modifier.fillMaxWidth(),
    )
}

/**
 * A text field bound to a setting. With a [placeholder] the label stays small
 * above an empty field, so what applies when it is empty shows without focus.
 */
@Composable
private fun SettingTextField(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    placeholder: String?,
    modifier: Modifier,
    hint: String? = null,
    lineLimits: TextFieldLineLimits = TextFieldLineLimits.SingleLine,
    textStyle: TextStyle = MaterialTheme.typography.bodyLarge,
    inputTransformation: InputTransformation? = null,
) {
    val state = rememberTextFieldState(value)
    val current by rememberUpdatedState(value)
    val report by rememberUpdatedState(onChange)
    // Outside changes (a reset, another window) replace the text; typing reports it.
    LaunchedEffect(value) {
        if (state.text.toString() != value) state.setTextAndPlaceCursorAtEnd(value)
    }
    LaunchedEffect(state) {
        snapshotFlow { state.text.toString() }.collect { if (it != current) report(it) }
    }
    val shown = placeholder?.takeIf { it.isNotBlank() }
    OutlinedTextField(
        state = state,
        label = { Text(label) },
        labelPosition = TextFieldLabelPosition.Attached(alwaysMinimize = shown != null),
        placeholder = shown?.let { { Text(it, maxLines = 1, overflow = TextOverflow.Ellipsis) } },
        supportingText = hint?.let { { Text(it) } },
        lineLimits = lineLimits,
        textStyle = textStyle,
        inputTransformation = inputTransformation,
        modifier = modifier,
    )
}

/** Wrapper command, pre-launch command and environment variables. */
@Composable
internal fun ProcessFields(
    wrapper: String,
    preLaunch: String,
    env: String,
    onWrapper: (String) -> Unit,
    onPreLaunch: (String) -> Unit,
    onEnv: (String) -> Unit,
    inherited: Triple<String, String, String>? = null,
) {
    OptionField(wrapper, onWrapper, stringResource(Res.string.settings_wrapper), stringResource(Res.string.settings_wrapper_hint),
        placeholder = inherited?.first, monospace = true)
    OptionField(preLaunch, onPreLaunch, stringResource(Res.string.settings_pre_launch), stringResource(Res.string.settings_pre_launch_hint),
        placeholder = inherited?.second, monospace = true)
    OptionField(env, onEnv, stringResource(Res.string.settings_env), stringResource(Res.string.settings_env_hint),
        placeholder = inherited?.third, singleLine = false, monospace = true)
}

/**
 * A Java runtime from [runtimes], or [defaultLabel] (null) for the automatic
 * or inherited choice.
 */
@Composable
internal fun JavaChoice(runtimes: List<JavaRuntime>, selected: String?, defaultLabel: String, onSelect: (String?) -> Unit) {
    val managedLabel = stringResource(Res.string.settings_java_downloaded)
    SelectField(
        value = selected ?: defaultLabel,
        label = stringResource(Res.string.settings_java),
        modifier = Modifier.fillMaxWidth(),
        leadingIcon = { Icon(painterResource(Res.drawable.ic_coffee), null) },
    ) { close ->
        DropdownMenuItem(
            text = { Text(defaultLabel) },
            onClick = {
                onSelect(null)
                close()
            },
        )
        runtimes.forEach { java ->
            DropdownMenuItem(
                text = {
                    Text(
                        if (java.managed) "Java ${java.major} · ${java.arch} · $managedLabel"
                        else "Java ${java.major} · ${java.arch} · ${java.path}"
                    )
                },
                onClick = {
                    onSelect(java.path)
                    close()
                },
            )
        }
    }
}

@Composable
internal fun SwitchRow(title: String, subtitle: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleSmall)
            Hint(subtitle)
        }
        Spacer(Modifier.width(12.dp))
        Switch(checked, onCheckedChange = onChange)
    }
}

@Composable
internal fun NumberField(label: String, value: UInt, modifier: Modifier, onChange: (UInt) -> Unit) {
    OutlinedTextField(
        value = value.toString(),
        onValueChange = { text -> text.filter(Char::isDigit).take(5).toUIntOrNull()?.let(onChange) },
        label = { Text(label) },
        singleLine = true,
        modifier = modifier,
    )
}

/** A number that may be unset (empty); [placeholder] shows what applies then. */
@Composable
internal fun OptionalNumberField(label: String, value: UInt?, placeholder: String, modifier: Modifier, onChange: (UInt?) -> Unit) {
    SettingTextField(
        value = value?.toString().orEmpty(),
        onChange = { text -> onChange(text.toUIntOrNull()) },
        label = label,
        placeholder = placeholder,
        modifier = modifier,
        inputTransformation = DigitsOnly,
    )
}

/** Up to six digits. */
private val DigitsOnly = InputTransformation {
    if (length > 6 || !asCharSequence().all(Char::isDigit)) revertAllChanges()
}

@Composable
internal fun Label(text: String) {
    Text(text, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

@Composable
internal fun Hint(text: String) {
    Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
}
