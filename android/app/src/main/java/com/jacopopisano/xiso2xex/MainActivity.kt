package com.jacopopisano.xiso2xex

import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            val xboxGreen = Color(0xFF107C10)
            val scheme = if (isSystemInDarkTheme()) {
                darkColorScheme(primary = Color(0xFF5DC21E), secondary = xboxGreen)
            } else {
                lightColorScheme(primary = xboxGreen, secondary = Color(0xFF0E6B0E))
            }
            MaterialTheme(colorScheme = scheme) {
                ConverterScreen(
                    keepScreenOn = { on ->
                        if (on) window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                        else window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                    },
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ConverterScreen(
    keepScreenOn: (Boolean) -> Unit,
    vm: ConverterViewModel = viewModel(),
) {
    val state by vm.state.collectAsStateWithLifecycle()

    val pickIso = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri?.let(vm::onIsoPicked)
    }
    val pickDest = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        uri?.let(vm::onDestinationPicked)
    }

    val extracting = state is UiState.Extracting
    DisposableEffect(extracting) {
        keepScreenOn(extracting)
        onDispose { keepScreenOn(false) }
    }

    Scaffold(topBar = { TopAppBar(title = { Text("ISO → XEX per Xbox") }) }) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            when (val s = state) {
                UiState.Idle -> IdleContent(onPick = { pickIso.launch(arrayOf("*/*")) })

                is UiState.Analyzing -> Row(verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator()
                    Spacer(Modifier.padding(8.dp))
                    Text("Analisi di ${s.isoName}…")
                }

                is UiState.Ready -> {
                    SummaryCard(s.isoName, s.summary)
                    if (s.summary.executable == null) {
                        Text(
                            "Attenzione: default.xex non trovato nella radice dell'ISO. " +
                                "Puoi comunque estrarre i file.",
                            color = MaterialTheme.colorScheme.error,
                        )
                    }
                    Button(onClick = { pickDest.launch(null) }, modifier = Modifier.fillMaxWidth()) {
                        Text("Scegli cartella ed estrai")
                    }
                    OutlinedButton(onClick = { pickIso.launch(arrayOf("*/*")) }, modifier = Modifier.fillMaxWidth()) {
                        Text("Scegli un'altra ISO")
                    }
                }

                is UiState.Extracting -> {
                    SummaryCard(s.isoName, s.summary)
                    val total = s.summary.totalSize.coerceAtLeast(1)
                    LinearProgressIndicator(
                        progress = { s.doneBytes.toFloat() / total },
                        modifier = Modifier.fillMaxWidth(),
                    )
                    Text("${formatBytes(s.doneBytes)} / ${formatBytes(s.summary.totalSize)}")
                    Text("File ${s.filesDone + 1} di ${s.summary.fileCount}: ${s.currentFile}",
                        style = MaterialTheme.typography.bodySmall)
                    OutlinedButton(onClick = vm::cancel, modifier = Modifier.fillMaxWidth()) {
                        Text("Annulla")
                    }
                }

                is UiState.Done -> {
                    Card(Modifier.fillMaxWidth()) {
                        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            Text("Conversione completata", style = MaterialTheme.typography.titleLarge)
                            Text("Cartella creata: ${s.folderName}")
                            Text("${s.summary.fileCount} file, ${formatBytes(s.summary.totalSize)}")
                        }
                    }
                    HowToUse()
                    Button(onClick = vm::reset, modifier = Modifier.fillMaxWidth()) {
                        Text("Converti un'altra ISO")
                    }
                }

                is UiState.Failed -> AlertDialog(
                    onDismissRequest = vm::dismissError,
                    confirmButton = { TextButton(onClick = vm::dismissError) { Text("OK") } },
                    title = { Text("Errore") },
                    text = { Text(s.message) },
                )
            }
        }
    }
}

@Composable
private fun IdleContent(onPick: () -> Unit) {
    Text(
        "Estrae il contenuto di un'ISO Xbox 360 (o Xbox originale) in una cartella " +
            "con default.xex, pronta da avviare dalla console.",
        style = MaterialTheme.typography.bodyLarge,
    )
    Button(onClick = onPick, modifier = Modifier.fillMaxWidth()) {
        Text("Scegli file .ISO")
    }
    HowToUse()
}

@Composable
private fun HowToUse() {
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text("Come usarla sulla Xbox", style = MaterialTheme.typography.titleMedium)
            Text("1. Copia la cartella estratta su una chiavetta USB o sull'HDD della console (es. Hdd1:/Games/).")
            Text("2. Avvia default.xex da una dashboard come Aurora, FreeStyle Dash o XeXMenu.")
            Text(
                "Gli XEX estratti da un'ISO si avviano solo su console modificate (RGH/JTAG). " +
                    "Usa solo copie di backup dei giochi che possiedi.",
                style = MaterialTheme.typography.bodySmall,
            )
        }
    }
}

@Composable
private fun SummaryCard(isoName: String, summary: IsoSummary) {
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(isoName, style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(4.dp))
            InfoRow("Formato", summary.layoutLabel)
            summary.executable?.let { exe ->
                InfoRow("Eseguibile", if (exe.format == "XBE") "default.xbe (Xbox)" else "default.xex")
                exe.titleName?.let { InfoRow("Titolo", it) }
                exe.titleId?.let { InfoRow("Title ID", it, mono = true) }
                exe.mediaId?.let { InfoRow("Media ID", it, mono = true) }
                exe.version?.let { InfoRow("Versione", it) }
                if (exe.discNumber != null && exe.discCount != null && exe.discCount > 0) {
                    InfoRow("Disco", "${exe.discNumber} di ${exe.discCount}")
                }
            }
            InfoRow("File", summary.fileCount.toString())
            InfoRow("Dimensione", formatBytes(summary.totalSize))
        }
    }
}

@Composable
private fun InfoRow(label: String, value: String, mono: Boolean = false) {
    Row(Modifier.fillMaxWidth()) {
        Text(label, Modifier.weight(0.4f), color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(
            value,
            Modifier.weight(0.6f),
            fontFamily = if (mono) FontFamily.Monospace else null,
        )
    }
}
