import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Rectangle {
 id:root
 required property var bridge
 readonly property var prompt:bridge.activeQuestion || null
 readonly property var state:bridge.questionState || ({})
 readonly property var questions:prompt?prompt.questions:[]
 readonly property int questionIndex:Math.max(0,Math.min(state.index || 0,questions.length-1))
 readonly property var question:questions[questionIndex] || ({id:"",options:[]})
 readonly property var answers:state.answers || ({})
 readonly property var answer:answers[question.id] || (question.initialAnswer!==undefined?{custom:question.initialAnswer}:{})
 readonly property var operation:prompt?(bridge.questionOperations[bridge.selectedKey+"/"+prompt.requestId] || {error:state.responseError || "",answered:state.responseStatus==="accepted"}):({})
 readonly property bool locked:!!operation.busy || !!operation.answered || !!state.attempt || !!bridge.draft.loading || !!bridge.draft.loadError || (prompt && prompt.responseCapability==="not_resumable")
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 property string collapsedQuestion:""
 readonly property bool collapsed:collapsedQuestion===question.id
 readonly property string identity:bridge.selectedKey+"/"+(prompt?prompt.requestId:"")+"/"+question.id
 onIdentityChanged:{autoAdvance.stop();collapsedQuestion=""}
 visible:!!prompt
 implicitHeight:body.implicitHeight+Style.space(12)
 radius:Style.space(10);color:Qt.rgba(fg.r,fg.g,fg.b,0.035);border.width:1;border.color:Qt.rgba(fg.r,fg.g,fg.b,0.12)
 Keys.onPressed:function(event){if(!collapsed&&!locked&&event.modifiers===Qt.NoModifier&&event.key>=Qt.Key_1&&event.key<=Qt.Key_9){choose(event.key-Qt.Key_1);event.accepted=true}}
 function resolved(q){
   var a=answers[q.id] || (q.initialAnswer!==undefined?{custom:q.initialAnswer}:{}),custom=q.allowCustomAnswer===false?null:(q.initialAnswer!==undefined?(a.custom!==undefined?a.custom:null):((a.custom || "").trim() || null))
   if(custom!==null && custom.length)return custom
   var selected=(a.selected || []).filter(function(v){return q.options.some(function(o){return (o.value!==undefined?o.value:o.label)===v})})
   return selected.length?(q.multiSelect?selected:selected[0]):custom
 }
 function choose(index){
   if(locked)return
   var option=question.options[index];if(!option)return
   var value=option.value!==undefined?option.value:option.label,selected=(answer.selected || []).slice()
   if(question.multiSelect){var i=selected.indexOf(value);if(i<0)selected.push(value);else selected.splice(i,1)}else selected=[value]
   var next=Object.assign({},answers);next[question.id]={selected:selected,custom:""};bridge.updateQuestion({answers:next})
   if(!question.multiSelect){autoAdvance.identity=root.identity;autoAdvance.restart()}
 }
 function custom(text){if(locked)return;autoAdvance.stop();var next=Object.assign({},answers);next[question.id]={custom:text};bridge.updateQuestion({answers:next})}
 function advance(){
   autoAdvance.stop()
   if(state.attempt){if(!operation.busy&&!operation.answered&&!bridge.draft.loading&&!bridge.draft.loadError&&prompt.responseCapability!=="not_resumable")bridge.respondQuestion(state.attempt.action,state.attempt.answers);return}
   if(locked || resolved(question)===null)return
   if(questionIndex<questions.length-1){bridge.updateQuestion({index:questionIndex+1});return}
   var result={};for(var i=0;i<questions.length;i++){var v=resolved(questions[i]);if(v===null){bridge.updateQuestion({index:i});return}result[questions[i].id]=v}
   bridge.respondQuestion("respond",result)
 }
 Timer{id:autoAdvance;property string identity:"";interval:200;onTriggered:if(identity===root.identity)root.advance()}
 Connections{target:root.bridge;ignoreUnknownSignals:true;function onQuestionSubmitRequested(){root.advance()}}
 ColumnLayout{
  id:body;anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(6)} spacing:Style.space(4)
  RowLayout{
   Layout.fillWidth:true
   T3IconButton{name:root.collapsed?"chevron-down":"chevron-up";tooltipText:T3I18n.tr("Question");onClicked:root.collapsedQuestion=root.collapsed?"":root.question.id}
   Text{Layout.fillWidth:true;text:root.question.header || T3I18n.tr("Question");color:root.dim;font.pixelSize:Style.font.caption;font.bold:true;textFormat:Text.PlainText;elide:Text.ElideRight}
   Text{text:(root.questionIndex+1)+"/"+root.questions.length;color:root.dim;font.pixelSize:Style.font.caption}
   T3IconButton{visible:!!root.prompt && root.prompt.dismissible;name:"x";tooltipText:T3I18n.tr("Dismiss question without answering");enabled:!root.operation.busy&&!root.operation.answered&&!root.state.attempt&&!root.bridge.draft.loading;onClicked:root.bridge.respondQuestion("dismiss",{})}
  }
  Controls.ScrollView{
   id:scroll;Layout.fillWidth:true;Layout.preferredHeight:Math.min(content.implicitHeight,Style.space(300));visible:!root.collapsed;clip:true
   contentWidth:availableWidth
   T3Wheel{flickable:scroll.contentItem}
   ColumnLayout{
    id:content;width:scroll.availableWidth;spacing:Style.space(3)
    Text{Layout.fillWidth:true;Layout.leftMargin:Style.space(8);Layout.rightMargin:Style.space(8);text:root.question.question || "";color:root.fg;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;textFormat:Text.MarkdownText}
    Text{Layout.fillWidth:true;Layout.leftMargin:Style.space(8);visible:!!root.question.multiSelect;text:T3I18n.tr("Select one or more options.");color:root.dim;font.pixelSize:Style.font.caption}
    Repeater{
     model:root.question.options || []
     delegate:Rectangle{
      id:option;required property var modelData;required property int index
      readonly property bool selected:(root.answer.selected || []).indexOf(modelData.value!==undefined?modelData.value:modelData.label)>=0 && !(root.answer.custom || "").length
      objectName:"questionOption"+index
      Layout.fillWidth:true;implicitHeight:labels.implicitHeight+Style.space(16);radius:Style.space(6)
      color:selected?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.1):mouse.containsMouse?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04):"transparent"
      enabled:!root.locked;activeFocusOnTab:true;Accessible.role:Accessible.Button;Accessible.name:modelData.label
      border.width:activeFocus?1:0;border.color:Color.accent
      Keys.onReturnPressed:root.choose(index)
      Keys.onSpacePressed:root.choose(index)
      Keys.onPressed:function(event){if(event.modifiers===Qt.NoModifier&&event.key>=Qt.Key_1&&event.key<=Qt.Key_9){root.choose(event.key-Qt.Key_1);event.accepted=true}}
      RowLayout{anchors.fill:parent;anchors.margins:Style.space(8);spacing:Style.space(8)
       ColumnLayout{id:labels;Layout.fillWidth:true;spacing:Style.space(2)
        Text{Layout.fillWidth:true;text:option.modelData.label;color:root.fg;font.bold:true;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;textFormat:Text.PlainText}
        Text{Layout.fillWidth:true;visible:!!option.modelData.description&&option.modelData.description!==option.modelData.label;text:option.modelData.description || "";color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
       }
       Text{text:option.selected?"✓":option.index<9?String(option.index+1):"";color:option.selected?Color.accent:root.dim;font.pixelSize:Style.font.caption}
      }
      MouseArea{id:mouse;anchors.fill:parent;hoverEnabled:true;cursorShape:Qt.PointingHandCursor;onClicked:{option.forceActiveFocus();root.choose(option.index)}}
     }
    }
    Controls.TextArea{
     id:customInput;objectName:"questionCustomAnswer";Layout.fillWidth:true;Layout.minimumHeight:Style.space(64)
     visible:root.question.allowCustomAnswer!==false;enabled:!root.locked
     text:root.answer.custom || "";placeholderText:T3I18n.tr("Write custom answer");color:root.fg;placeholderTextColor:root.dim;font.pixelSize:Style.font.body;wrapMode:TextEdit.Wrap;textFormat:TextEdit.PlainText;selectByMouse:true
     background:Rectangle{radius:Style.space(6);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.025);border.width:1;border.color:customInput.activeFocus?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.1)}
     onTextChanged:if(activeFocus && text!==(root.answer.custom || ""))root.custom(text)
     Keys.onReturnPressed:function(event){if(event.modifiers===Qt.MetaModifier){root.advance();event.accepted=true}else event.accepted=false}
    }
   }
  }
  RowLayout{
   Layout.fillWidth:true
   T3IconButton{objectName:"questionPrevious";name:"chevron-left";tooltipText:T3I18n.tr("Previous question");enabled:root.questionIndex>0&&!root.locked;onClicked:{autoAdvance.stop();root.bridge.updateQuestion({index:root.questionIndex-1})}}
   Text{Layout.fillWidth:true;text:root.operation.error?T3I18n.error(root.operation.error):root.operation.busy?T3I18n.tr("Sending…"):root.operation.answered?T3I18n.tr("Answered"):root.state.attempt?T3I18n.tr("Response pending confirmation"):(root.prompt&&root.prompt.responseCapability==="not_resumable"?T3I18n.tr("This question can no longer be answered."):"");color:root.operation.error?Color.urgent:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   T3IconButton{objectName:"questionRetry";visible:!!root.state.attempt&&!root.operation.busy&&!root.operation.answered;name:"refresh-cw";tooltipText:T3I18n.tr("Retry same action");onClicked:root.bridge.respondQuestion(root.state.attempt.action,root.state.attempt.answers)}
   T3IconButton{objectName:"questionNext";name:root.questionIndex===root.questions.length-1?"arrow-up":"chevron-right";tooltipText:root.state.attempt?T3I18n.tr("Retry same action"):root.questionIndex===root.questions.length-1?T3I18n.tr("Send"):T3I18n.tr("Next question");enabled:root.state.attempt?(!root.operation.busy&&!root.operation.answered&&!root.bridge.draft.loading&&!root.bridge.draft.loadError&&root.prompt.responseCapability!=="not_resumable"):(!root.locked&&root.resolved(root.question)!==null);onClicked:root.advance()}
  }
 }
}
