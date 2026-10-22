import QtQuick
import QtQuick.Window 6.11
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
import qs.Commons
ShellRoot{
 id:root;property bool done:false
 function check(ok,msg){if(!ok){console.error("T3_QUESTIONS_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 Timer{interval:400;running:true;onTriggered:checks.runChecks()}
 Timer{interval:150;running:root.done;onTriggered:Qt.quit()}
 QtObject{
  id:fixture
  property string selectedKey:"fixture/thread"
  property var draft:({loading:false,loadError:""})
  property var questionOperations:({})
  property var questionState:({})
  property var activeQuestion:({requestId:"test-request",responseCapability:"live",dismissible:false,questions:[{id:"when",header:"Timing",question:"**When** should it happen?",options:[{label:"Before expiry",description:"Once per idle period",value:"before "},{label:"Never",description:"Manual only",value:"never"}]},{id:"providers",header:"Providers",question:"Which?",multiSelect:true,allowCustomAnswer:false,options:[{label:"Codex",value:"codex"},{label:"Other",value:"other"}]},{id:"detail",header:"Details",question:"Anything else?",options:[],initialAnswer:"Existing text"}]})
  property int submittedCount:0
  property var submitted:null
  signal questionSubmitRequested()
  function updateQuestion(values){questionState=Object.assign({},questionState,values)}
  function respondQuestion(action,answers){submittedCount++;submitted={action:action,answers:answers}}
 }
 Window{
  id:win;visible:true;width:760;height:700;color:Color.popups.background
  Rectangle{anchors.fill:parent;color:Color.popups.background}
  Test.T3Questions{id:card;width:720;anchors.centerIn:parent;bridge:fixture}
  TestCase{
   id:checks;when:false
   function runChecks(){
    win.requestActivate();wait(150)
    root.check(card.visible&&card.question.id==="when","pending question displayed")
    var first=findChild(card,"questionOption0");mouseClick(first,20,first.height/2);wait(230)
    root.check(card.question.id==="providers"&&card.answers.when.selected[0]==="before ","native single option values preserved and auto advance")
    card.choose(0);card.choose(1);wait(230);root.check(card.question.id==="providers","multi-select does not auto advance")
    card.choose(1);card.advance();root.check(card.question.id==="detail","next question")
    mouseClick(findChild(card,"questionPrevious"),12,12);root.check(card.question.id==="providers","previous button retains multi answer");card.advance()
    root.check(card.resolved(card.question)==="Existing text","editable initial answer seeded")
    var input=findChild(card,"questionCustomAnswer");input.forceActiveFocus();keyClick(Qt.Key_A,Qt.ControlModifier);for(var ch of "typed")keyClick(ch.toUpperCase().charCodeAt(0));wait(60)
    root.check(card.resolved(card.question)==="typed","free text editing")
    keyClick(Qt.Key_Return,Qt.ControlModifier);root.check(fixture.submitted.action==="respond"&&fixture.submitted.answers.when==="before "&&fixture.submitted.answers.providers[0]==="codex"&&fixture.submitted.answers.detail==="typed","one native response with all answers")
    var sent=fixture.submitted.answers;fixture.submitted=null;fixture.updateQuestion({attempt:{action:"respond",answers:sent,requestId:"same-command"},responseStatus:"failed",responseError:"invalid_runtime_request"});wait(40)
    var send=findChild(card,"questionNext");root.check(card.locked&&send.enabled,"failed saved attempt keeps text locked but enables primary retry")
    root.check(card.operation.error==="invalid_runtime_request","saved failure survives lost in-memory operation state")
    mouseClick(send,send.width/2,send.height/2);root.check(fixture.submitted&&JSON.stringify(fixture.submitted.answers)===JSON.stringify(sent),"primary retry reuses original saved answer")
    fixture.submitted=null;fixture.updateQuestion({attempt:null,responseStatus:"",responseError:"",index:0});card.choose(1);fixture.selectedKey="fixture/other";wait(250)
    root.check(fixture.submitted===null,"delayed auto advance cannot submit on another thread")
    fixture.activeQuestion=Object.assign({},fixture.activeQuestion,{responseCapability:"not_resumable"});root.check(card.locked,"expired provider request cannot answer")
    fixture.activeQuestion=Object.assign({},fixture.activeQuestion,{responseCapability:"message",dismissible:true});root.check(!card.locked,"async message-mode request can answer")
    card.width=340;wait(120);root.check(card.height<600&&card.height>100,"narrow layout bounded")
    win.contentItem.grabToImage(function(result){result.saveToFile("/tmp/t3-questions.png")});wait(150)
    fixture.activeQuestion={requestId:"manual-before-timer",responseCapability:"live",questions:[{id:"one",header:"One",question:"Choose",options:[{label:"A",description:""}]}]};fixture.questionState=({});fixture.submittedCount=0
    card.choose(0);card.advance();wait(260);root.check(fixture.submittedCount===1,"manual submit cancels delayed choice advance and never retries implicitly")
    fixture.activeQuestion=null;root.check(!card.visible,"resolved request disappears")
    win.visible=false;root.done=true;console.log("T3_QUESTIONS_QML_PASS: native choice values, multi select, custom answers, navigation, submit, thread switch and capability guards")
   }
  }
 }
}
