package com.nts.viewer.protocol

internal actual fun environmentVariable(name: String): String? = System.getenv(name)
