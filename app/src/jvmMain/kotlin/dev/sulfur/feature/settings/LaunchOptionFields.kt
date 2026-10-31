package dev.sulfur.feature.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExposedDropdownMenu
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
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
import dev.sulfur.resources.isolation_default
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
        label = { gc -> gc?.let { gcLabel(it) } ?: stringResource(Res.string.isolation_default) },
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
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        placeholder = placeholder?.takeIf { it.isNotBlank() }?.let { { Text(it, maxLines = 1) } },
        supportingText = hint?.let { { Text(it) } },
        singleLine = singleLine,
        minLines = if (singleLine) 1 else 3,
        maxLines = if (singleLine) 1 else 8,
        textStyle = if (monospace) MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace)
        else MaterialTheme.typography.bodyLarge,
        modifier = Modifier.fillMaxWidth(),
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
    var open by remember { mutableStateOf(false) }
    val managedLabel = stringResource(Res.string.settings_java_downloaded)
    ExposedDropdownMenuBox(expanded = open, onExpandedChange = { open = it }) {
        OutlinedTextField(
            value = selected ?: defaultLabel,
            onValueChange = {},
            readOnly = true,
            label = { Text(stringResource(Res.string.settings_java)) },
            leadingIcon = { Icon(painterResource(Res.drawable.ic_coffee), null) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(open) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
        )
        ExposedDropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            DropdownMenuItem(
                text = { Text(defaultLabel) },
                onClick = {
                    onSelect(null)
                    open = false
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
                        open = false
                    },
                )
            }
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
    OutlinedTextField(
        value = value?.toString().orEmpty(),
        onValueChange = { text -> onChange(text.filter(Char::isDigit).take(6).toUIntOrNull()) },
        label = { Text(label) },
        placeholder = { Text(placeholder, maxLines = 1) },
        singleLine = true,
        modifier = modifier,
    )
}

@Composable
internal fun Label(text: String) {
    Text(text, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

@Composable
internal fun Hint(text: String) {
    Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
}
