import SwiftUI
import MapLibre

struct ContentView: View {
    @State private var styleURL: URL?
    @State private var errorMessage: String?
    @State private var mapState: MapState?  // Keep alive to maintain tile server

    var body: some View {
        ZStack {
            if let styleURL = styleURL {
                MapLibreMapView(styleURL: styleURL)
            } else if let error = errorMessage {
                VStack {
                    Text("Failed to load map")
                        .font(.headline)
                    Text(error)
                        .font(.caption)
                        .foregroundColor(.red)
                        .padding()
                }
            } else {
                ProgressView("Loading map...")
            }
        }
        .ignoresSafeArea()
        .background(Color.white)
        .task {
            await loadStyle()
        }
    }

    private func loadStyle() async {
        do {
            let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!.path

            initPanicHandler()

            let viewState = ViewState(basePath: documentsPath)
            let state = try await viewState.getMapState()
            let styleJson = state.getStyle()

            guard let styleData = styleJson.data(using: .utf8) else {
                errorMessage = "Failed to convert style JSON to data"
                return
            }

            let tempURL = FileManager.default.temporaryDirectory.appendingPathComponent("style.json")
            try styleData.write(to: tempURL)

            // Store mapState to keep tile server running
            mapState = state
            styleURL = tempURL
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

struct MapLibreMapView: UIViewRepresentable {
    let styleURL: URL

    func makeUIView(context: Context) -> MLNMapView {
        let mapView = MLNMapView(frame: .zero, styleURL: styleURL)
        mapView.delegate = context.coordinator
        mapView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        mapView.backgroundColor = .white

        mapView.setCenter(
            CLLocationCoordinate2D(latitude: 40.7571418, longitude: -73.9805655),
            zoomLevel: 12,
            animated: false
        )

        mapView.compassView.isHidden = true
        mapView.logoView.isHidden = true
        mapView.attributionButton.isHidden = true

        return mapView
    }

    func updateUIView(_ mapView: MLNMapView, context: Context) {
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    class Coordinator: NSObject, MLNMapViewDelegate {
        func mapViewDidFailLoadingMap(_ mapView: MLNMapView, withError error: Error) {
            print("Map failed to load: \(error)")
        }
    }
}

#Preview {
    ContentView()
}
