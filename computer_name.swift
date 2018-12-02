#!/usr/bin/env swift

import Foundation

class ComputerName

if let deviceName = Host.current().localizedName {
   print(deviceName)
}
