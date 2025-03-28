FROM rockylinux:9

# Install requirements : update repo and install all requirements
RUN yum clean all && \
  rm -rf /var/cache/yum && rm -rf /var/cache/dnf && \
  yum install -y yum-utils && \
  yum-config-manager --add-repo https://cli.github.com/packages/rpm/gh-cli.repo && \
  yum-config-manager --enable crb && \
  yum install -y --allowerasing gcc gcc-c++ \
    git make python39 bash coreutils gh \
    rpm-build rpm-devel rpmlint diffutils patch rpmdevtools \
    cmake ninja-build pkg-config golang \
    gtk3-devel gtk4-devel qt5-qtbase-devel \
    zlib-devel curl-devel && \
  yum clean all && \
  rm -rf /var/cache/yum && rm -rf /var/cache/dnf

# Install cmake 3.29.8
RUN curl -L https://github.com/Kitware/CMake/releases/download/v3.29.8/cmake-3.29.8-linux-x86_64.tar.gz | \
  tar -C /usr/local --strip-components=1 --gz -xf - && \
  ln -sf cmake /usr/local/bin/cmake3 && \
  cmake --version && \
  cmake3 --version
