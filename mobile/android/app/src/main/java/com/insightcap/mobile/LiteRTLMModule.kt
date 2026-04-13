package com.insightcap.mobile

import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.bridge.Promise
import com.facebook.react.modules.core.DeviceEventManagerModule

class LiteRTLMModule(private val reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

    override fun getName(): String = "LiteRTLM"

    @ReactMethod
    fun isModelLoaded(promise: Promise) {
        // TODO: 實作模型載入狀態檢查
        promise.resolve(false)
    }

    @ReactMethod
    fun loadModel(modelPath: String, maxTokens: Int, promise: Promise) {
        // TODO: 透過 LiteRT-LM SDK 載入 Gemma 4 .task 模型
        promise.reject("E_NOT_IMPL", "LiteRT-LM model loading not yet implemented")
    }

    @ReactMethod
    fun unloadModel(promise: Promise) {
        // TODO: 卸載模型釋放記憶體
        promise.resolve(null)
    }

    @ReactMethod
    fun generateStream(prompt: String, requestId: String) {
        // TODO: 呼叫 LiteRT-LM 推理，透過 event 回傳 token
        // 目前回傳錯誤事件
        val params = com.facebook.react.bridge.Arguments.createMap().apply {
            putString("requestId", requestId)
            putString("error", "LiteRT-LM 尚未實作，請使用雲端或桌面模式")
        }
        reactContext
            .getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter::class.java)
            .emit("LiteRTLM_onError", params)
    }

    @ReactMethod
    fun addListener(eventName: String) {
        // Required for RN event emitter
    }

    @ReactMethod
    fun removeListeners(count: Int) {
        // Required for RN event emitter
    }
}
