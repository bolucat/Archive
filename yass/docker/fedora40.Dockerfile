FROM fedora:40

# Install requirements : update repo and install all requirements
RUN yum clean all && \
  rm -rf /var/cache/yum && rm -rf /var/cache/dnf && \
  yum install -y yum-utils && \
  yum-config-manager --add-repo https://cli.github.com/packages/rpm/gh-cli.repo && \
  yum install -y gcc gcc-c++ \
    git make python3 bash coreutils gh systemd \
    rpm-build rpm-devel rpmlint diffutils patch rpmdevtools \
    cmake ninja-build pkg-config perl golang \
    gtk3-devel gtk4-devel qt5-qtbase-devel qt6-qtbase-devel \
    zlib-devel c-ares-devel libnghttp2-devel curl-devel \
    json-devel mbedtls-devel gperftools-devel jsoncpp-devel && \
  yum clean all && \
  rm -rf /var/cache/yum && rm -rf /var/cache/dnf

# Install cmake 3.29.8
RUN curl -L https://github.com/Kitware/CMake/releases/download/v3.29.8/cmake-3.29.8-linux-x86_64.tar.gz | \
  tar -C /usr/local --strip-components=1 --gz -xf - && \
  ln -sf cmake /usr/local/bin/cmake3 && \
  cmake --version && \
  cmake3 --version
