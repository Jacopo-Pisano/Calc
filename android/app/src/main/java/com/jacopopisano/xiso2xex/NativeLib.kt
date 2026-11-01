package com.jacopopisano.xiso2xex

import java.io.IOException

/** Funzioni implementate in Rust (core/src/jni_api.rs). */
object NativeLib {
    init {
        System.loadLibrary("xiso")
    }

    /** Analizza l'ISO aperta su [fd] e restituisce il riepilogo in JSON. */
    @JvmStatic
    @Throws(IOException::class)
    external fun summarize(fd: Int): String

    /**
     * Copia [length] byte a partire da [offset] dell'ISO ([inFd]) in coda al
     * file aperto su [outFd]. Restituisce i byte copiati.
     */
    @JvmStatic
    @Throws(IOException::class)
    external fun copyRange(inFd: Int, offset: Long, length: Long, outFd: Int): Long
}
