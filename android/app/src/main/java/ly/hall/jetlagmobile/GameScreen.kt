package ly.hall.jetlagmobile

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import ly.hall.jetlagmobile.ui.theme.JetLagMobileTheme
import org.maplibre.android.MapLibre
import org.maplibre.android.maps.MapLibreMapOptions
import org.maplibre.android.maps.MapView
import org.maplibre.android.maps.Style
import uniffi.jet_lag_mobile.ViewState

class GameScreen : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        MapLibre.getInstance(this)
        enableEdgeToEdge()
        setContent { JetLagMobileTheme { MapLibreMap(modifier = Modifier.fillMaxSize()) } }
    }
}

@Composable
fun MapLibreMap(modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current

    // Create ViewState from Rust
    val viewState = remember { ViewState() }
    val styleJson = remember(viewState) { viewState.getStyle() }

    val mapView = remember {
        val options =
                MapLibreMapOptions.createFromAttributes(context).apply {
                    compassEnabled(false)
                    // need attribution on a splash screen tho
                    attributionEnabled(false)
                    logoEnabled(false)
                }

        MapView(context, options).apply {
            getMapAsync { map -> map.setStyle(Style.Builder().fromJson(styleJson)) }
        }
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
            viewState.destroy()
        }
    }

    AndroidView(factory = { mapView }, modifier = modifier)
}
