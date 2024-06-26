// Copyright 2023 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.jni_zero;

import org.jni_zero.NativeMethods;

class SampleModule {
    void test() {
        if (SampleForAnnotationProcessorJni.get().bar(1)) {
            SampleForAnnotationProcessorJni.get().foo();
        }
    }

    @NativeMethods("module")
    interface Natives {
        void foo();
        boolean bar(int a);
    }
}
