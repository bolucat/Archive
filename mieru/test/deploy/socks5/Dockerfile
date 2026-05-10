# Copyright (C) 2026  mieru authors
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

# Assume this file is executed with `docker build -f` from the root dir of mieru project.

FROM ubuntu:24.04

WORKDIR /test

# Copy binaries and test script into the container.
COPY bin/httpserver bin/udpserver bin/socks5server bin/sockshttpclient bin/socksudpclient \
    test/deploy/socks5/test.sh /test/

CMD ["/test/test.sh"]
