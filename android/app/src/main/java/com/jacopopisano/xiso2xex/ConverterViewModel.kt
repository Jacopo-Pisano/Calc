package com.jacopopisano.xiso2xex

import android.app.Application
import android.net.Uri
import android.provider.OpenableColumns
import androidx.documentfile.provider.DocumentFile
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.IOException
import kotlin.coroutines.cancellation.CancellationException
import kotlin.coroutines.coroutineContext

sealed interface UiState {
    data object Idle : UiState
    data class Analyzing(val isoName: String) : UiState
    data class Ready(val isoName: String, val summary: IsoSummary) : UiState
    data class Extracting(
        val isoName: String,
        val summary: IsoSummary,
        val currentFile: String,
        val doneBytes: Long,
        val filesDone: Int,
    ) : UiState
    data class Done(val folderName: String, val summary: IsoSummary) : UiState
    data class Failed(val message: String, val previous: UiState) : UiState
}

class ConverterViewModel(app: Application) : AndroidViewModel(app) {
    private val _state = MutableStateFlow<UiState>(UiState.Idle)
    val state: StateFlow<UiState> = _state.asStateFlow()

    private var isoUri: Uri? = null
    private var job: Job? = null

    private val resolver get() = getApplication<Application>().contentResolver

    fun onIsoPicked(uri: Uri) {
        job?.cancel()
        isoUri = uri
        val name = displayName(uri) ?: "gioco.iso"
        _state.value = UiState.Analyzing(name)
        job = viewModelScope.launch {
            _state.value = try {
                val summary = withContext(Dispatchers.IO) {
                    resolver.openFileDescriptor(uri, "r")?.use { pfd ->
                        IsoSummary.fromJson(NativeLib.summarize(pfd.fd))
                    } ?: throw IOException("Impossibile aprire il file")
                }
                UiState.Ready(name, summary)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                UiState.Failed(e.message ?: e.toString(), UiState.Idle)
            }
        }
    }

    fun onDestinationPicked(treeUri: Uri) {
        val ready = _state.value as? UiState.Ready ?: return
        val iso = isoUri ?: return
        job = viewModelScope.launch {
            try {
                val folder = withContext(Dispatchers.IO) { extract(iso, treeUri, ready) }
                _state.value = UiState.Done(folder, ready.summary)
            } catch (e: CancellationException) {
                // Annullato dall'utente: torna alla schermata di riepilogo
                // (a meno che nel frattempo non sia stato fatto reset()).
                if (_state.value is UiState.Extracting) _state.value = ready
                throw e
            } catch (e: Exception) {
                _state.value = UiState.Failed(e.message ?: e.toString(), ready)
            }
        }
    }

    fun cancel() {
        job?.cancel()
    }

    fun dismissError() {
        (_state.value as? UiState.Failed)?.let { _state.value = it.previous }
    }

    fun reset() {
        job?.cancel()
        isoUri = null
        _state.value = UiState.Idle
    }

    private suspend fun extract(iso: Uri, treeUri: Uri, ready: UiState.Ready): String {
        val ctx = getApplication<Application>()
        val root = DocumentFile.fromTreeUri(ctx, treeUri)
            ?: throw IOException("Cartella di destinazione non valida")
        val summary = ready.summary
        val folderName = folderNameFor(ready)
        val outRoot = root.createDirectory(folderName)
            ?: throw IOException("Impossibile creare la cartella $folderName")
        val dirs = mutableMapOf("" to outRoot)

        fun dirFor(path: String): DocumentFile {
            dirs[path]?.let { return it }
            val parent = dirFor(path.substringBeforeLast('/', ""))
            val name = path.substringAfterLast('/')
            val created = parent.createDirectory(name)
                ?: throw IOException("Impossibile creare la cartella $path")
            dirs[path] = created
            return created
        }

        var doneBytes = 0L
        var filesDone = 0
        resolver.openFileDescriptor(iso, "r")?.use { input ->
            for (entry in summary.entries) {
                coroutineContext.ensureActive()
                if (entry.isDir) {
                    dirFor(entry.path)
                    continue
                }
                _state.value = UiState.Extracting(ready.isoName, summary, entry.path, doneBytes, filesDone)
                val parent = dirFor(entry.path.substringBeforeLast('/', ""))
                val name = entry.path.substringAfterLast('/')
                val file = parent.createFile("application/octet-stream", name)
                    ?: throw IOException("Impossibile creare ${entry.path}")
                resolver.openFileDescriptor(file.uri, "w")?.use { output ->
                    var copied = 0L
                    while (copied < entry.size) {
                        coroutineContext.ensureActive()
                        val chunk = minOf(CHUNK, entry.size - copied)
                        copied += NativeLib.copyRange(input.fd, entry.offset + copied, chunk, output.fd)
                        _state.value = UiState.Extracting(
                            ready.isoName, summary, entry.path, doneBytes + copied, filesDone,
                        )
                    }
                } ?: throw IOException("Impossibile scrivere ${entry.path}")
                doneBytes += entry.size
                filesDone++
            }
        } ?: throw IOException("Impossibile riaprire l'ISO")
        return folderName
    }

    private fun folderNameFor(ready: UiState.Ready): String {
        val base = ready.isoName.substringBeforeLast('.').ifBlank { "gioco" }
        val id = ready.summary.executable?.titleId
        val raw = if (id != null) "$base [$id]" else base
        return raw.replace(Regex("[\\\\/:*?\"<>|]"), "_")
    }

    private fun displayName(uri: Uri): String? =
        resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        }

    private companion object {
        const val CHUNK = 16L * 1024 * 1024
    }
}
