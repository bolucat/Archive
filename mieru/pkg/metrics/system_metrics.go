// Copyright (C) 2021  mieru authors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

package metrics

const (
	userMetricGroupPrefix = "user - "
)

const (
	UserMetricGroupFormat = userMetricGroupPrefix + "%s"

	UserMetricUploadBytes   = "UploadBytes"
	UserMetricDownloadBytes = "DownloadBytes"
)

var (
	// Max number of connections ever reached.
	MaxConn = RegisterMetric("connections", "MaxConn", GAUGE)

	// Accumulated active open connections.
	ActiveOpens = RegisterMetric("connections", "ActiveOpens", COUNTER)

	// Accumulated passive open connections.
	PassiveOpens = RegisterMetric("connections", "PassiveOpens", COUNTER)

	// Current number of established connections.
	CurrEstablished = RegisterMetric("connections", "CurrEstablished", GAUGE)

	// Number of bytes from client to server.
	UploadBytes = RegisterMetric("traffic", "UploadBytes", COUNTER)

	// Number of bytes from server to client.
	DownloadBytes = RegisterMetric("traffic", "DownloadBytes", COUNTER)

	// Number of padding bytes send to proxy connections.
	OutputPaddingBytes = RegisterMetric("traffic", "OutputPaddingBytes", COUNTER)
)
