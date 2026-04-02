package com.example.flutter_gameboy

import android.content.Context
import android.os.Bundle
import io.flutter.embedding.android.FlutterActivity

class MainActivity : FlutterActivity() {
	companion object {
		init {
			System.loadLibrary("rust_lib_flutter_gameboy")
		}
	}

	private external fun initNativeContext(context: Context)

	override fun onCreate(savedInstanceState: Bundle?) {
		super.onCreate(savedInstanceState)
		initNativeContext(applicationContext)
	}
}
