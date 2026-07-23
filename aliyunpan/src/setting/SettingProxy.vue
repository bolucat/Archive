<script setup lang="ts">
import { ref } from 'vue'
import useSettingStore from './settingstore'
import MySwitch from '../layout/MySwitch.vue'
import message from '../utils/message'
/*import HttpsProxyAgent from 'https-proxy-agent'
import { SocksProxyAgent } from 'socks-proxy-agent'*/
import AliHttp from '../aliapi/alihttp'
import nodehttps from 'node:https'
import { t } from '../i18n'

const settingStore = useSettingStore()
const cb = (val: any) => {
  if (!Object.hasOwn(val, 'proxyUseProxy') && settingStore.proxyUseProxy) {
    val.proxyUseProxy = false
  }
  settingStore.updateStore(val)
}
const proxyLoading = ref(false)

const handleProxyConn = async () => {
  proxyLoading.value = true

  let option = {
    strictSSL: false,
    rejectUnauthorized: false,
    timeout: 5000
  }
  const proxy = settingStore.getProxy()
  if (proxy) {
    /*if (settingStore.proxyType.startsWith('http')) {
      const agenth = HttpsProxyAgent(proxy)
      option = Object.assign(option, { agent: agenth })
    } else {
      const agents = new SocksProxyAgent(proxy)
      option = Object.assign(option, { agent: agents })
    }*/

    const result = await new Promise<string>(async (resolve) => {
      nodehttps
        .get(AliHttp.baseApi, option, (res: any) => {
          resolve('success')
        })
        .on('error', (e: any) => {
          let message = e.message || e.code || t('settings.proxy.networkError')
          message = message.replace('ERR_SSL_INVALID_LIBRARY_(0)', t('settings.proxy.unsupportedCert'))
          message = message.replace('A "socket" was not created for HTTP request before 5000ms', t('settings.proxy.timeout'))
          message = message.replace('Client network socket disconnected before secure TLS connection was established', t('settings.proxy.tlsFailed'))
          resolve(message)
        })
    })
    if (result == 'success') {
      message.success(t('settings.proxy.success'))
    } else {
      message.error(`${t('settings.proxy.error')} ${result}`)
    }
  } else {
    message.error(t('settings.proxy.error'))
  }
  proxyLoading.value = false
}
</script>

<template>
  <div class="settingcard">
    <div class="settings-proxy-intro">
      <div class="settings-proxy-kicker">Network</div>
      <div class="settings-proxy-copy">{{ t('settings.proxy.intro') }}</div>
    </div>
    <div class="settinghead">{{ t('settings.proxy.type') }}</div>
    <div class="settingrow">
      <a-select tabindex="-1" :style="{ width: '168px' }" :model-value="settingStore.proxyType" :popup-container="'#SettingDiv'" @update:model-value="cb({ proxyType: $event })">
        <a-option value="none">None</a-option>
        <a-option value="http">HTTP</a-option>
        <a-option value="https">HTTPS</a-option>
        <a-option value="socks5">SOCKS5</a-option>
        <a-option value="socks5h">SOCKS5H</a-option>
      </a-select>
      <a-popover position="right">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            <span class="opred">{{ t('settings.proxy.defaultType') }}</span>
            <hr />
            {{ t('settings.proxy.typeTip') }}
          </div>
        </template>
      </a-popover>
    </div>
    <div class="settingspace"></div>

    <a-row class="grid-demo">
      <a-col flex="252px">
        <div class="settinghead">{{ t('settings.proxy.host') }}</div>
        <div class="settingrow">
          <a-input tabindex="-1" :style="{ width: '168px' }" :placeholder="t('settings.proxy.hostPlaceholder')" :model-value="settingStore.proxyHost" @update:model-value="cb({ proxyHost: $event })" />
        </div>
      </a-col>
      <a-col flex="180px">
        <div class="settinghead">{{ t('settings.proxy.port') }}</div>
        <div class="settingrow">
          <a-input-number tabindex="-1" :style="{ width: '168px' }" hide-button :placeholder="t('settings.proxy.portPlaceholder')" :model-value="settingStore.proxyPort" @update:model-value="cb({ proxyPort: $event })" />
        </div>
      </a-col>
      <a-col flex="auto"> </a-col>
    </a-row>
    <div class="settingspace"></div>
    <a-row class="grid-demo">
      <a-col flex="252px">
        <div class="settinghead">{{ t('settings.proxy.username') }}</div>
        <div class="settingrow">
          <a-input tabindex="-1" :style="{ width: '168px' }" :placeholder="t('settings.proxy.optional')" :model-value="settingStore.proxyUserName" @update:model-value="cb({ proxyUserName: $event })" />
        </div>
      </a-col>
      <a-col flex="180px">
        <div class="settinghead">{{ t('settings.proxy.password') }}</div>
        <div class="settingrow">
          <a-input tabindex="-1" :style="{ width: '168px' }" :placeholder="t('settings.proxy.optional')" :model-value="settingStore.proxyPassword" @update:model-value="cb({ proxyPassword: $event })" />
        </div>
      </a-col>
      <a-col flex="auto"> </a-col>
    </a-row>

    <div class="settingspace"></div>
    <div class="settingrow">
      <a-button type="primary" size="small" tabindex="-1" :loading="proxyLoading" @click="handleProxyConn">{{ t('settings.proxy.test') }}</a-button>
      <span style="margin-left: 8px; font-size: 12px; color: var(--color-text-3)">{{ t('settings.proxy.testFirst') }}</span>
    </div>
    <div class="settingspace"></div>
    <div class="settinghead">{{ t('settings.proxy.enable') }}</div>
    <div class="settingrow">
      <MySwitch :value="settingStore.proxyUseProxy" @update:value="cb({ proxyUseProxy: $event })">{{ t('settings.proxy.useProxy') }}</MySwitch>
      <a-popover position="right">
        <IconFont name="iconbulb" />
        <template #content>
          <div>
            <span class="opred">{{ t('settings.defaultOff') }}</span>
            <hr />
            {{ t('settings.proxy.enableTip') }}
          </div>
        </template>
      </a-popover>
    </div>
  </div>
</template>

<style scoped>
.settings-proxy-intro {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 14px;
}

.settings-proxy-kicker {
  display: inline-flex;
  align-self: flex-start;
  padding: 4px 10px;
  border-radius: 999px;
  background: rgba(88, 130, 255, 0.12);
  color: var(--color-primary-6);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.settings-proxy-copy {
  max-width: 580px;
  color: var(--color-text-2);
  font-size: 14px;
  line-height: 1.7;
}

:global(html.dark) .settings-proxy-kicker {
  background: rgba(120, 160, 255, 0.2);
  color: #dbe6ff;
}
</style>
