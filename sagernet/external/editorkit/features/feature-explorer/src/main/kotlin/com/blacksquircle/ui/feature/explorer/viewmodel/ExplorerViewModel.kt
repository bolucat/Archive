/*
 * Copyright 2021 Squircle IDE contributors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package com.blacksquircle.ui.feature.explorer.viewmodel

import android.util.Log
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.blacksquircle.ui.data.storage.keyvalue.SettingsManager
import com.blacksquircle.ui.domain.model.editor.DocumentModel
import com.blacksquircle.ui.domain.repository.explorer.ExplorerRepository
import com.blacksquircle.ui.feature.explorer.R
import com.blacksquircle.ui.feature.explorer.utils.Operation
import com.blacksquircle.ui.feature.explorer.utils.replaceList
import com.blacksquircle.ui.filesystem.base.exception.*
import com.blacksquircle.ui.filesystem.base.model.FileModel
import com.blacksquircle.ui.filesystem.base.model.FileTree
import com.blacksquircle.ui.filesystem.base.model.PropertiesModel
import com.blacksquircle.ui.utils.event.SingleLiveEvent
import com.blacksquircle.ui.utils.extensions.launchEvent
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.flow.onCompletion
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class ExplorerViewModel @Inject constructor(
    private val settingsManager: SettingsManager,
    private val explorerRepository: ExplorerRepository
) : ViewModel() {

    companion object {
        private const val TAG = "ExplorerViewModel"
    }

    // region EVENTS

    val toastEvent = SingleLiveEvent<Int>() // ?????????????????????? ??????????????????

    val showAppBarEvent = MutableLiveData(false) // ?????????????????????? ??????????????
    val allowPasteFiles = MutableLiveData(false) // ?????????????????????? ???????????? "????????????????"
    val loadingBar = MutableLiveData(true) // ?????????????????? ???????????????? ????????????
    val emptyView = MutableLiveData(false) // ?????????????????? ???? ???????????????????? ????????????

    val filesUpdateEvent = SingleLiveEvent<Unit>() // ???????????? ???? ???????????????????? ???????????? ????????????
    val selectAllEvent = SingleLiveEvent<Unit>() // ???????????????? ?????? ??????????
    val deselectAllEvent = SingleLiveEvent<Unit>() // ???????????????? ?????????????????? ???? ???????? ????????????
    val createEvent = SingleLiveEvent<Unit>() // ?????????????? ????????
    val copyEvent = SingleLiveEvent<Unit>() // ?????????????????????? ???????????????????? ??????????
    val deleteEvent = SingleLiveEvent<Unit>() // ?????????????? ???????????????????? ??????????
    val cutEvent = SingleLiveEvent<Unit>() // ???????????????? ???????????????????? ??????????
    val pasteEvent = SingleLiveEvent<Unit>() // ???????????????? ?????????????????????????? ??????????
    val openAsEvent = SingleLiveEvent<Unit>() // ?????????????? ???????? ??????
    val renameEvent = SingleLiveEvent<Unit>() // ?????????????????????????? ????????
    val propertiesEvent = SingleLiveEvent<Unit>() // ???????????????? ??????????
    val copyPathEvent = SingleLiveEvent<Unit>() // ?????????????????????? ???????? ?? ??????????
    val archiveEvent = SingleLiveEvent<Unit>() // ?????????????????? ???????????? ?? .zip

    val tabEvent = MutableLiveData<FileModel>() // ?????????????????? ?????????????????????? ??????????????
    val selectionEvent = MutableLiveData<List<FileModel>>() // ???????????? ???????????????????? ????????????

    val progressEvent = SingleLiveEvent<Int>() // ???????????????? ???????????????????? ????????????????
    val filesEvent = SingleLiveEvent<FileTree>() // ???????????? ????????????
    val searchEvent = SingleLiveEvent<List<FileModel>>() // ???????????????????????????? ???????????? ????????????
    val clickEvent = SingleLiveEvent<FileModel>() // ???????????????? ?????????????? ???? ????????
    val propertiesOfEvent = SingleLiveEvent<PropertiesModel>() // ???????????????? ??????????

    val openFileEvent = SingleLiveEvent<DocumentModel>() // ???????????????? ?????????? ???? ???????????????????? ?? ??????????????????
    val openPropertiesEvent = SingleLiveEvent<FileModel>() // ???????????????? ?????????????? ???????????????????? ??????????

    // endregion EVENTS

    val tabsList = mutableListOf<FileModel>()
    val tempFiles = mutableListOf<FileModel>()

    var operation = Operation.COPY
    var currentJob: Job? = null

    var viewMode: Int
        get() = settingsManager.viewMode.toInt()
        set(value) {
            settingsManager.viewMode = value.toString()
            filesUpdateEvent.call()
        }
    var showHidden: Boolean
        get() = settingsManager.filterHidden
        set(value) {
            settingsManager.filterHidden = value
            filesUpdateEvent.call()
        }
    var sortMode: Int
        get() = settingsManager.sortMode.toInt()
        set(value) {
            settingsManager.sortMode = value.toString()
            filesUpdateEvent.call()
        }

    private val searchList = mutableListOf<FileModel>()

    fun provideDirectory(path: String?) {
        viewModelScope.launchEvent(loadingBar) {
            try {
                emptyView.value = false

                val fileTree = explorerRepository.fetchFiles(path?.let(::FileModel))
                tabEvent.value = fileTree.parent
                filesEvent.value = fileTree
                searchList.replaceList(fileTree.children)

                emptyView.value = fileTree.children.isEmpty()
            } catch (e: Exception) {
                Log.e(TAG, e.message, e)
                when (e) {
                    is DirectoryExpectedException -> {
                        toastEvent.value = R.string.message_directory_expected
                    }
                    else -> {
                        toastEvent.value = R.string.message_unknown_exception
                    }
                }
            }
        }
    }

    fun searchFile(query: String) {
        val collection = mutableListOf<FileModel>()
        if (query.isEmpty()) {
            collection.addAll(searchList)
        } else {
            for (row in searchList) {
                if (row.name.contains(query, ignoreCase = true)) {
                    collection.add(row)
                }
            }
        }
        emptyView.value = collection.isEmpty()
        searchEvent.value = collection
    }

    fun createFile(fileModel: FileModel) {
        viewModelScope.launch {
            try {
                val file = explorerRepository.createFile(fileModel)
                filesUpdateEvent.call()
                clickEvent.value = file
                toastEvent.value = R.string.message_done
            } catch (e: Exception) {
                Log.e(TAG, e.message, e)
                when (e) {
                    is FileAlreadyExistsException -> {
                        toastEvent.value = R.string.message_file_already_exists
                    }
                    else -> {
                        toastEvent.value = R.string.message_unknown_exception
                    }
                }
            }
        }
    }

    fun renameFile(fileModel: FileModel, newName: String) {
        viewModelScope.launch {
            try {
                explorerRepository.renameFile(fileModel, newName)
                filesUpdateEvent.call()
                toastEvent.value = R.string.message_done
            } catch (e: Exception) {
                Log.e(TAG, e.message, e)
                when (e) {
                    is FileNotFoundException -> {
                        toastEvent.value = R.string.message_file_not_found
                    }
                    is FileAlreadyExistsException -> {
                        toastEvent.value = R.string.message_file_already_exists
                    }
                    else -> {
                        toastEvent.value = R.string.message_unknown_exception
                    }
                }
            }
        }
    }

    fun propertiesOf(fileModel: FileModel) {
        viewModelScope.launch {
            try {
                val properties = explorerRepository.propertiesOf(fileModel)
                propertiesOfEvent.value = properties
            } catch (e: Exception) {
                Log.e(TAG, e.message, e)
                when (e) {
                    is FileNotFoundException -> {
                        toastEvent.value = R.string.message_file_not_found
                    }
                    else -> {
                        toastEvent.value = R.string.message_unknown_exception
                    }
                }
            }
        }
    }

    fun deleteFiles(fileModels: List<FileModel>) {
        currentJob = viewModelScope.launch {
            progressEvent.value = 0
            try {
                explorerRepository.deleteFiles(fileModels)
                    .onEach {
                        progressEvent.value = (progressEvent.value ?: 0) + 1
                    }
                    .onCompletion {
                        filesUpdateEvent.call()
                        toastEvent.value = R.string.message_done
                    }
                    .collect()
            } catch (e: Exception) {
                Log.e(TAG, e.message, e)
                progressEvent.value = Int.MAX_VALUE
                when (e) {
                    is FileNotFoundException -> {
                        toastEvent.value = R.string.message_file_not_found
                    }
                    is CancellationException -> {
                        toastEvent.value = R.string.message_operation_cancelled
                    }
                    else -> {
                        toastEvent.value = R.string.message_unknown_exception
                    }
                }
            }
        }
    }

    fun copyFiles(source: List<FileModel>, destPath: String) {
        currentJob = viewModelScope.launch {
            progressEvent.value = 0
            try {
                explorerRepository.copyFiles(source, destPath)
                    .onEach {
                        progressEvent.value = (progressEvent.value ?: 0) + 1
                    }
                    .onCompletion {
                        filesUpdateEvent.call()
                        toastEvent.value = R.string.message_done
                    }
                    .collect()
            } catch (e: Exception) {
                Log.e(TAG, e.message, e)
                progressEvent.value = Int.MAX_VALUE
                when (e) {
                    is FileNotFoundException -> {
                        toastEvent.value = R.string.message_file_not_found
                    }
                    is FileAlreadyExistsException -> {
                        toastEvent.value = R.string.message_file_already_exists
                    }
                    is CancellationException -> {
                        toastEvent.value = R.string.message_operation_cancelled
                    }
                    else -> {
                        toastEvent.value = R.string.message_unknown_exception
                    }
                }
            }
        }
    }

    fun cutFiles(source: List<FileModel>, destPath: String) {
        currentJob = viewModelScope.launch {
            progressEvent.value = 0
            try {
                explorerRepository.cutFiles(source, destPath)
                    .onEach {
                        progressEvent.value = (progressEvent.value ?: 0) + 1
                    }
                    .onCompletion {
                        filesUpdateEvent.call()
                        toastEvent.value = R.string.message_done
                    }
                    .collect()
            } catch (e: Exception) {
                Log.e(TAG, e.message, e)
                progressEvent.value = Int.MAX_VALUE
                when (e) {
                    is FileNotFoundException -> {
                        toastEvent.value = R.string.message_file_not_found
                    }
                    is FileAlreadyExistsException -> {
                        toastEvent.value = R.string.message_file_already_exists
                    }
                    is CancellationException -> {
                        toastEvent.value = R.string.message_operation_cancelled
                    }
                    else -> {
                        toastEvent.value = R.string.message_unknown_exception
                    }
                }
            }
        }
    }

    fun compressFiles(source: List<FileModel>, destPath: String, archiveName: String) {
        currentJob = viewModelScope.launch {
            progressEvent.value = 0
            try {
                val dest = FileModel("$destPath/$archiveName")
                explorerRepository.compressFiles(source, dest)
                    .onEach {
                        progressEvent.value = (progressEvent.value ?: 0) + 1
                    }
                    .onCompletion {
                        filesUpdateEvent.call()
                        toastEvent.value = R.string.message_done
                    }
                    .collect()
            } catch (e: Exception) {
                Log.e(TAG, e.message, e)
                progressEvent.value = Int.MAX_VALUE
                when (e) {
                    is FileNotFoundException -> {
                        toastEvent.value = R.string.message_file_not_found
                    }
                    is FileAlreadyExistsException -> {
                        toastEvent.value = R.string.message_file_already_exists
                    }
                    is CancellationException -> {
                        toastEvent.value = R.string.message_operation_cancelled
                    }
                    else -> {
                        toastEvent.value = R.string.message_unknown_exception
                    }
                }
            }
        }
    }

    fun extractAll(source: FileModel, destPath: String) {
        currentJob = viewModelScope.launch {
            progressEvent.value = 0
            try {
                val dest = FileModel(destPath)
                explorerRepository.extractAll(source, dest)
                    .onEach {
                        progressEvent.value = (progressEvent.value ?: 0) + 1
                    }
                    .onCompletion {
                        filesUpdateEvent.call()
                        toastEvent.value = R.string.message_done
                    }
                    .collect()
            } catch (e: Exception) {
                Log.e(TAG, e.message, e)
                progressEvent.value = Int.MAX_VALUE
                when (e) {
                    is FileNotFoundException -> {
                        toastEvent.value = R.string.message_file_not_found
                    }
                    is FileAlreadyExistsException -> {
                        toastEvent.value = R.string.message_file_already_exists
                    }
                    is UnsupportedArchiveException -> {
                        toastEvent.value = R.string.message_unsupported_archive
                    }
                    is EncryptedArchiveException -> {
                        toastEvent.value = R.string.message_encrypted_archive
                    }
                    is SplitArchiveException -> {
                        toastEvent.value = R.string.message_split_archive
                    }
                    is InvalidArchiveException -> {
                        toastEvent.value = R.string.message_invalid_archive
                    }
                    is CancellationException -> {
                        toastEvent.value = R.string.message_operation_cancelled
                    }
                    else -> {
                        toastEvent.value = R.string.message_unknown_exception
                    }
                }
            }
        }
    }
}