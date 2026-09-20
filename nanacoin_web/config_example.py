"""Copy this file to config.py and fill in your details.

config.py is gitignored so credentials stay out of version control. It also
never reaches the network: static.py refuses any request path containing a
`..` segment, which is the rule that keeps this file off the web server that
sits beside it.
"""

# The S2 has no 5GHz radio. If this SSID is 5GHz-only where the board sits,
# the board cannot see the network at all and reports it exactly as it reports
# a wrong password.
WIFI_SSID = "your-network-name"
WIFI_PASSWORD = "your-password"
