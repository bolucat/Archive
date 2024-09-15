import { CdnConfig } from './types'

const owner = 'the1812'
const host = 'cdn.jsdelivr.net'
export const jsDelivr: CdnConfig = {
  name: 'jsDelivr',
  owner,
  host,
  stableClient: `https://${host}/gh/${owner}/Bilibili-Evolved@master/dist/bilibili-evolved.user.js`,
  previewClient: `https://${host}/gh/${owner}/Bilibili-Evolved@preview/dist/bilibili-evolved.preview.user.js`,
  library: {
    lodash: {
      url: `https://${host}/npm/lodash@4.17.21/lodash.min.js`,
      sha256: 'a9705dfc47c0763380d851ab1801be6f76019f6b67e40e9b873f8b4a0603f7a9',
    },
    protobuf: {
      url: `https://${host}/npm/protobufjs@6.10.1/dist/light/protobuf.min.js`,
      sha256: '8978daf871b02d683ecaee371861702a6f31d0a4c52925b7db2bb1655a8bc7d1',
    },
    jszip: {
      url: `https://${host}/npm/jszip@3.7.1/dist/jszip.min.js`,
      sha256: 'c9e4a52bac18aee4f3f90d05fbca603f5b0f5bf1ce8c45e60bb4ed3a2cb2ed86',
    },
    sortable: {
      url: `https://${host}/npm/sortablejs@1.14.0/Sortable.min.js`,
      sha256: '0ea5a6fbfbf5434b606878533cb7a66bcf700f0f08afe908335d0978fb63ad94',
    },
    streamsaver: {
      url: `https://${host}/npm/streamsaver@2.0.6/StreamSaver.min.js`,
      sha256: '64f465e51e5992be894c5d42330b781544eda5462069fe6be4c7421f02d28c92',
    },
    ffmpeg: {
      worker: {
        url: `https://${host}/npm/@ffmpeg/ffmpeg@0.12.4/dist/umd/814.ffmpeg.js`,
        sha256: 'baf19437171b1bccae4416e4da69fb40455b8e67142f79c8ec9da36b1de7fd8a',
      },
      core: {
        url: `https://${host}/npm/@ffmpeg/core@0.12.4/dist/umd/ffmpeg-core.js`,
        sha256: '6af6b8cd8c878dec6f61f3cd6be16e88f9391dd265e51f20afea5c0f718bfba0',
      },
      wasm: {
        url: `https://${host}/npm/@ffmpeg/core@0.12.4/dist/umd/ffmpeg-core.wasm`,
        sha256: '925bd7ef35d4e0f715254cd650f4cfc68c4ec6ecebf293face72b92da904ddda',
      },
    },
  },
  smallLogo: `https://${host}/gh/${owner}/Bilibili-Evolved@preview/images/logo-small.png`,
  logo: `https://${host}/gh/${owner}/Bilibili-Evolved@preview/images/logo.png`,
  root: (branch, ownerOverride) =>
    `https://${host}/gh/${ownerOverride || owner}/Bilibili-Evolved@${branch}/`,
}
