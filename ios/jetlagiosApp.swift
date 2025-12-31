//
//  jetlagiosApp.swift
//  jetlagios
//
//  Created by Mac Studio on 12/30/25.
//

import SwiftUI

@main
struct jetlagiosApp: App {
    let persistenceController = PersistenceController.shared

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(\.managedObjectContext, persistenceController.container.viewContext)
        }
    }
}
