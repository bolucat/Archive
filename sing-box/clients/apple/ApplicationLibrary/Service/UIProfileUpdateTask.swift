import BackgroundTasks
import Foundation
import Library
#if canImport(UIKit)
    import UIKit
#endif

#if os(iOS) || os(tvOS)
    public class UIProfileUpdateTask: BGAppRefreshTask {
        private static let taskSchedulerPermittedIdentifier = "\(FilePath.packageName).update_profiles"

        private static var registered = false
        public static func configure() throws {
            if !registered {
                let success = BGTaskScheduler.shared.register(forTaskWithIdentifier: taskSchedulerPermittedIdentifier, using: nil) { task in
                    NSLog("profile update task started")
                    Task {
                        await UIProfileUpdateTask.getAndUpdateProfiles(task)
                    }
                }
                if !success {
                    throw NSError(domain: "register task failed", code: 0)
                }
                registered = true
            }
            Task {
                BGTaskScheduler.shared.cancelAllTaskRequests()
                let profiles = try await ProfileManager.listAutoUpdateEnabled()
                if profiles.isEmpty {
                    return
                }
                try scheduleUpdate(ProfileUpdateTask.calculateEarliestBeginDate(profiles))
            }
            Task {
                if await UIApplication.shared.backgroundRefreshStatus != .available {
                    await updateOnce()
                }
            }
        }

        private nonisolated static func updateOnce() async {
            NSLog("update profiles at start since background refresh unavailable")
            let profiles: [Profile]
            do {
                profiles = try await ProfileManager.listAutoUpdateEnabled()
            } catch {
                return
            }
            if profiles.isEmpty {
                return
            }
            _ = await ProfileUpdateTask.updateProfiles(profiles)
        }

        private nonisolated static func getAndUpdateProfiles(_ task: BGTask) async {
            let profiles: [Profile]
            do {
                profiles = try await ProfileManager.listAutoUpdateEnabled()
            } catch {
                return
            }
            if profiles.isEmpty {
                return
            }
            let success = await ProfileUpdateTask.updateProfiles(profiles)
            try? scheduleUpdate(ProfileUpdateTask.calculateEarliestBeginDate(profiles))
            task.setTaskCompleted(success: success)
            task.expirationHandler = {
                try? scheduleUpdate(nil)
            }
        }

        private static func scheduleUpdate(_ earliestBeginDate: Date?) throws {
            let request = BGAppRefreshTaskRequest(identifier: taskSchedulerPermittedIdentifier)
            request.earliestBeginDate = earliestBeginDate
            try BGTaskScheduler.shared.submit(request)
        }
    }
#endif
