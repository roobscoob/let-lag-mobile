package ly.hall.jetlagmobile

import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import ly.hall.jetlagmobile.ui.theme.JetLagMobileTheme
import org.maplibre.android.MapLibre
import org.maplibre.android.camera.CameraPosition
import org.maplibre.android.geometry.LatLng
import org.maplibre.android.maps.MapLibreMap
import org.maplibre.android.maps.MapLibreMapOptions
import org.maplibre.android.maps.MapView
import org.maplibre.android.maps.Style
import org.maplibre.android.style.layers.CustomLayer
import uniffi.jet_lag_mobile.MapState
import uniffi.jet_lag_mobile.UniffiLib
import uniffi.jet_lag_mobile.ViewState
import uniffi.jet_lag_mobile.uniffiEnsureInitialized

class GameScreen : ComponentActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    MapLibre.getInstance(this)
    Thread.sleep(1000)

    // Force connectivity to always be true
    forceConnectivity(this)

    enableEdgeToEdge()
    setContent { JetLagMobileTheme { MapLibreMap(modifier = Modifier.fillMaxSize()) } }
  }

  private fun forceConnectivity(context: android.content.Context) {
    try {
      val receiverClass = Class.forName("org.maplibre.android.net.ConnectivityReceiver")
      val instanceMethod =
        receiverClass.getMethod("instance", android.content.Context::class.java)
      val receiver = instanceMethod.invoke(null, context)

      // Use setConnected method (it exists!)
      val setConnectedMethod =
        receiverClass.getDeclaredMethod("setConnected", Boolean::class.javaObjectType)
      setConnectedMethod.isAccessible = true
      setConnectedMethod.invoke(receiver, true)

      Log.i("ConnectivityFix", "Successfully forced connectivity to true")

      // Verify it worked
      val isConnectedMethod = receiverClass.getMethod("isConnected")
      val currentState = isConnectedMethod.invoke(receiver) as Boolean
      Log.i("ConnectivityFix", "Current connected state: $currentState")
    } catch (e: Exception) {
      Log.e("ConnectivityFix", "Error forcing connectivity", e)
    }
  }

}

object CustomLayerShim {
  init {
    uniffiEnsureInitialized();
    System.loadLibrary("custom-layer-shim")
  }

  external fun getCustomLayer(kind: Int): Long
}

@Composable
fun MapLibreMap(modifier: Modifier = Modifier) {
  val context = LocalContext.current
  val lifecycleOwner = LocalLifecycleOwner.current
  val viewState = remember { ViewState(context.filesDir.absolutePath) }
  var mapState by remember { mutableStateOf<MapState?>(null) }
  var map by remember { mutableStateOf<MapLibreMap?>(null) }

  LaunchedEffect(viewState) { mapState = viewState.getMapState() }

  LaunchedEffect(map, mapState) {
    val m = map ?: return@LaunchedEffect
    val ms = mapState ?: return@LaunchedEffect

    val layer = CustomLayerShim.getCustomLayer(0)

    m.setStyle(Style.Builder()
      .fromJson(ms.getStyle())
      .withLayer(CustomLayer("transit-lines", layer))
    )
  }

  val mapView = remember {
    val options =
      MapLibreMapOptions.createFromAttributes(context).apply {
        compassEnabled(false)
        // need attribution on a splash screen tho
        attributionEnabled(false)
        logoEnabled(false)
        // Set initial camera to Central Park, NYC
        camera(
          CameraPosition.Builder()
            .target(LatLng(40.7571418, -73.9805655))
            .zoom(12.0)
            .build()
        )
      }

    MapView(context, options).apply { getMapAsync { map = it } }
  }

  DisposableEffect(lifecycleOwner) {
    val observer = LifecycleEventObserver { _, event ->
      when (event) {
        Lifecycle.Event.ON_START -> mapView.onStart()
        Lifecycle.Event.ON_RESUME -> mapView.onResume()
        Lifecycle.Event.ON_PAUSE -> mapView.onPause()
        Lifecycle.Event.ON_STOP -> mapView.onStop()
        Lifecycle.Event.ON_DESTROY -> mapView.onDestroy()
        else -> {}
      }
    }

    lifecycleOwner.lifecycle.addObserver(observer)

    onDispose {
      lifecycleOwner.lifecycle.removeObserver(observer)
      mapView.onDestroy()
      mapState?.destroy()
      viewState.destroy()
    }
  }

  AndroidView(factory = { mapView }, modifier = modifier)
}
