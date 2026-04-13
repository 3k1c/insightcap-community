package com.insightcap.mobile

import android.content.Intent
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.WritableNativeMap

class ShareIntentModule(private val reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

    override fun getName(): String = "ShareIntentModule"

    @ReactMethod
    fun getSharedData(promise: Promise) {
        val currentActivity = currentActivity
        if (currentActivity == null) {
            promise.reject("E_NO_ACTIVITY", "No current activity available")
            return
        }

        val intent = currentActivity.intent
        val action = intent?.action
        val type = intent?.type

        if (Intent.ACTION_SEND == action && type != null) {
            val map = WritableNativeMap()

            if ("text/plain" == type) {
                val sharedText = intent.getStringExtra(Intent.EXTRA_TEXT)
                if (sharedText != null) {
                    map.putString("type", "text")
                    map.putString("value", sharedText)
                    promise.resolve(map)

                    intent.removeExtra(Intent.EXTRA_TEXT)
                    intent.action = Intent.ACTION_MAIN
                    return
                }
            } else if (type.startsWith("image/")) {
                val imageUri = intent.getParcelableExtra<android.net.Uri>(Intent.EXTRA_STREAM)
                if (imageUri != null) {
                    map.putString("type", "image")
                    map.putString("value", imageUri.toString())
                    promise.resolve(map)

                    intent.removeExtra(Intent.EXTRA_STREAM)
                    intent.action = Intent.ACTION_MAIN
                    return
                }
            }
        }

        promise.resolve(null)
    }
}
