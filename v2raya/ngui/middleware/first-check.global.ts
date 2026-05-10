export default defineNuxtRouteMiddleware(async() => {
  if (user.value.firstCheck) {
    // 使用 /api/version 接口的 hasAccounts 字段判断账户是否存在
    // 避免发送伪造的注册请求
    const { data } = await useV2Fetch<any>('version').get().json()
    user.value.firstCheck = false
    user.value.exist = data.value?.data?.hasAccounts === true
  }
})
