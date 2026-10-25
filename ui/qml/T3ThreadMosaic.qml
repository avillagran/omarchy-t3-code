import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/SidebarModel.js" as Sidebar

// Thread mosaic: up to eight live conversations side by side, drawn with the
// chat's own pieces (bubbles, rendered markdown, activity lines) over the panel
// sky. Each tile says what its agent is doing right now, with its own colour
// and motion: thinking breathes, reading scans, editing stitches, commands
// blink, a response types; input requests glint like their sidebar row.
Item {
  id: root;objectName:"t3ThreadMosaic"
  required property var bridge
  property var threads: []
  // Panel sky (shared clock, motion mode) and Preferences > Animations.
  property var sky: null
  property var effects: ({working:"comet",input:"glints",change:"star",intensity:1})
  // The chat's markdown renderer, so tiles read exactly like the conversation.
  property var renderer: null
  signal closed()
  signal filtersRequested()
  readonly property color fg: Color.popups.text
  readonly property color dim: Qt.rgba(fg.r,fg.g,fg.b,0.57)
  readonly property bool skyMoving: !!sky && sky.animated
  readonly property int count: Math.max(1,Math.min(8,bridge.viewState.mosaicCount || 4))
  property int page: 0
  readonly property int pages: Math.max(1,Math.ceil(threads.length/count))
  readonly property var shown: threads.slice(Math.min(page,pages-1)*count,Math.min(page,pages-1)*count+count)
  readonly property string targetKeys: shown.map(function(t){return t.key}).sort().join("\n")
  // Each thread keeps its slot while it stays on the page, so a reorder in
  // the list (a thread updating) never shuffles, replays or reloads tiles.
  property var slotKeys: []
  readonly property var byKey: {var m={};shown.forEach(function(t){m[t.key]=t});return m}
  function assignSlots(){
    var want={},placed={},next=[],i
    shown.forEach(function(t){want[t.key]=true})
    for(i=0;i<count;i++){var k=slotKeys[i];if(k&&want[k]&&!placed[k]){next.push(k);placed[k]=true}else next.push("")}
    var free=shown.filter(function(t){return !placed[t.key]})
    for(i=0;i<count&&free.length;i++)if(!next[i])next[i]=free.shift().key
    if(next.join("\n")!==slotKeys.join("\n"))slotKeys=next
  }
  onTargetKeysChanged: {assignSlots();sync.restart()}
  onCountChanged: {page=0;assignSlots()}
  onPagesChanged: if(page>=pages)page=pages-1
  Timer {id:sync;interval:50;onTriggered:root.bridge.watchMosaic(root.shown)}

  Component.onDestruction: bridge.watchMosaic([])

  // What a tile's agent is doing: the newest live activity, else a streaming
  // response, else the thread's sidebar state.
  function phaseOf(thread,snap){
    if(!thread)return "empty"
    var d=Sidebar.detail(thread)
    if(d==="approval"||d==="input"||d==="plan"||(!!thread.pending&&["request","plan","waiting"].indexOf(thread.reason)>=0))return "attention"
    if(d==="failed"||d==="limited")return "failed"
    var rows=snap.messages || [],last=rows.length?rows[rows.length-1]:null
    var live=(snap.activity || [])[0] || (last&&last.role==="activity"&&last.activity&&last.activity.active?last.activity:null)
    if(last&&last.role==="assistant"&&last.streaming===true)return "respond"
    if(live){
      var c=live.category || ({reasoning:"think",command_execution:"execute",file_change:"edit",file_search:"search",web_search:"web",subagent:"agent"})[live.type] || "tool"
      return ({think:"think",read:"read",search:"read",web:"read",edit:"edit",execute:"execute",agent:"agent"})[c] || "work"
    }
    if(thread.working)return "think"
    return d==="done"?"done":"idle"
  }
  function toneOf(phase){
    // Every active phase wears T3's working blue; the glyph and label tell them apart.
    return ({think:T3Palette.working,read:T3Palette.working,edit:T3Palette.working,execute:T3Palette.working,respond:T3Palette.working,
             agent:T3Palette.working,work:T3Palette.working,attention:T3Palette.waiting,failed:T3Palette.failed,done:T3Palette.done})[phase] || dim
  }
  function liveActivity(snap){var rows=snap.messages || [],last=rows.length?rows[rows.length-1]:null;return (snap.activity || [])[0] || (last&&last.role==="activity"&&last.activity&&last.activity.active?last.activity:null)}
  function labelOf(phase,thread,snap){
    if(!thread)return ""
    if(phase==="respond")return T3I18n.tr("Responding")
    if(phase==="think")return T3I18n.tr("Thinking")
    if(phase==="attention"||phase==="failed"||phase==="done"){
      var label=({approval:"Approval",input:"Input",plan:"Plan ready",limited:"Limited",failed:"Failed",done:"Done"})[Sidebar.detail(thread)]
      return T3I18n.tr(label || (phase==="attention"?"Waiting":phase==="failed"?"Failed":"Done"))
    }
    if(phase==="idle")return T3I18n.tr("Idle")
    var a=liveActivity(snap);return a?bridge.activityText(a):T3I18n.tr("Working")
  }
  function environmentName(id){var e=(bridge.state.environments || []).find(function(e){return e.id===id});return e?(e.name || e.label || e.id):id}
  readonly property var tally: {
    var t={live:0,attention:0,done:0}
    threads.forEach(function(th){var d=Sidebar.detail(th);if(d==="approval"||d==="input"||d==="plan")t.attention++;else if(th.working)t.live++;else if(d==="done")t.done++})
    return t
  }

  // A small icon with the motion of its phase.
  component PhaseGlyph: Item {
    id: glyph
    property string phase: "idle"
    property color tint: "gray"
    property bool live: false
    implicitWidth: Style.space(16);implicitHeight:implicitWidth
    T3Tick {id:glyphTick;running:glyph.live&&glyph.visible;pace:"smooth"}
    readonly property real t: glyphTick.time
    T3Icon {
      id:glyphIcon;anchors.centerIn:parent;visible:glyph.phase!=="respond"
      implicitWidth:glyph.width*0.9;tint:glyph.tint
      name:({think:"brain",read:"book-open",edit:"pen-line",execute:"terminal",agent:"bot",work:"loader-circle",attention:"message-circle-question",failed:"circle-alert",done:"circle-check",idle:"circle-dashed",empty:"layout-grid"})[glyph.phase] || "circle-dashed"
      scale:!glyph.live?1:glyph.phase==="think"?1+0.13*Math.sin(glyph.t*3.2):glyph.phase==="attention"?1+0.1*Math.sin(glyph.t*2.6):1
      opacity:glyph.live&&glyph.phase==="think"?0.72+0.28*Math.sin(glyph.t*3.2):1
      rotation:!glyph.live?0:glyph.phase==="edit"?14*Math.sin(glyph.t*7):glyph.phase==="work"?(glyph.t*240)%360:0
      transform:Translate{y:glyph.live&&(glyph.phase==="read"||glyph.phase==="agent")?Style.spaceReal(1.4)*Math.sin(glyph.t*4.5):0}
    }
    // A terminal cursor that blinks under the prompt.
    Rectangle {visible:glyph.live&&glyph.phase==="execute";x:glyph.width*0.55;y:glyph.height*0.74;width:glyph.width*0.34;height:Math.max(1.5,glyph.height*0.11);radius:height/2;color:glyph.tint;opacity:Math.floor(glyph.t*2.4)%2?0.15:1}
    // Typing dots for a response being written.
    Row {
      anchors.centerIn:parent;visible:glyph.phase==="respond";spacing:glyph.width*0.1
      Repeater {
        model:3
        Rectangle {
          required property int index
          width:glyph.width*0.2;height:width;radius:width/2;color:glyph.tint
          transform:Translate{y:glyph.live?-Math.abs(Math.sin(glyph.t*5.5-index*0.75))*glyph.height*0.22:0}
          opacity:glyph.live?0.45+0.55*Math.abs(Math.sin(glyph.t*5.5-index*0.75)):0.7
        }
      }
    }
  }

  // A hairline under the tile header that moves the way the phase works.
  component PhaseStrip: Item {
    id: strip
    property string phase: "idle"
    property color tint: "gray"
    property bool live: false
    implicitHeight: Style.spaceReal(2)
    clip: true
    T3Tick {id:stripTick;running:strip.live&&strip.visible&&strip.width>0;pace:"smooth"}
    readonly property real t: stripTick.time
    Rectangle {anchors.fill:parent;radius:height/2;color:Qt.rgba(strip.tint.r,strip.tint.g,strip.tint.b,strip.live?0.14:0.07)}
    // think and attention breathe; failed and done stay lit.
    Rectangle {
      anchors.fill:parent;radius:height/2;color:strip.tint
      visible:strip.phase==="think"||strip.phase==="attention"||strip.phase==="failed"||strip.phase==="done"
      opacity:!strip.live?(strip.phase==="done"?0.35:0.7):0.25+0.6*(0.5+0.5*Math.sin(strip.t*(strip.phase==="think"?3.2:2.6)))
    }
    // read, agent, work: a scanner sweeps across.
    Rectangle {
      visible:strip.live&&(strip.phase==="read"||strip.phase==="agent"||strip.phase==="work")
      width:strip.width*0.32;height:parent.height;radius:height/2
      x:-width+((strip.t/(strip.phase==="read"?1.3:2.1))%1)*(strip.width+width)
      gradient:Gradient{orientation:Gradient.Horizontal;GradientStop{position:0;color:"transparent"}GradientStop{position:0.7;color:strip.tint}GradientStop{position:1;color:Qt.lighter(strip.tint,1.35)}}
    }
    // respond: a line that writes itself from the left.
    Rectangle {
      visible:strip.live&&strip.phase==="respond"
      readonly property real p:(strip.t/1.6)%1
      width:strip.width*Math.min(1,p*1.25);height:parent.height;radius:height/2
      opacity:p<0.8?1:1-(p-0.8)/0.2
      gradient:Gradient{orientation:Gradient.Horizontal;GradientStop{position:0;color:Qt.rgba(strip.tint.r,strip.tint.g,strip.tint.b,0.25)}GradientStop{position:1;color:strip.tint}}
    }
    // edit: stitches marching along.
    Repeater {
      model:strip.live&&strip.phase==="edit"?Math.ceil(strip.width/Style.space(14))+1:0
      Rectangle {
        required property int index
        width:Style.space(7);height:strip.height;radius:height/2;color:strip.tint
        x:index*Style.space(14)-((strip.t*Style.space(28))%Style.space(14))
      }
    }
    // execute: blocks light up in turn, like a busy terminal.
    Row {
      visible:strip.live&&strip.phase==="execute";anchors.fill:parent;spacing:Style.space(3)
      Repeater {
        model:6
        Rectangle {
          required property int index
          width:(strip.width-5*Style.space(3))/6;height:strip.height;radius:height/2;color:strip.tint
          opacity:Math.floor(strip.t*7)%6===index?1:0.18
        }
      }
    }
  }

  // Motion: Preferences > Animations ("off" or the instant transition keeps
  // the mosaic still) and its speed.
  property bool motion: true
  function ms(n){return motion?Math.round(n*T3Motion.pace):0}
  // Page turns flip the tiles toward the direction of travel.
  property int flipDir: 0
  function turn(step){if(page+step<0||page+step>=pages)return;flipDir=step;page+=step;flipReset.restart()}
  Timer {id:flipReset;interval:root.ms(900)+50;onTriggered:root.flipDir=0}
  // Closing plays the mosaic out before the chat comes back.
  function dismiss(){if(!motion){closed();return}if(!leave.running)leave.start()}
  SequentialAnimation {
    id:leave
    ParallelAnimation {
      NumberAnimation{target:board;property:"opacity";to:0;duration:root.ms(240);easing.type:Easing.InCubic}
      NumberAnimation{target:board;property:"scale";to:0.96;duration:root.ms(260);easing.type:Easing.InCubic}
      NumberAnimation{target:header;property:"opacity";to:0;duration:root.ms(200)}
      NumberAnimation{target:headerShift;property:"y";to:-Style.space(14);duration:root.ms(220);easing.type:Easing.InCubic}
    }
    ScriptAction{script:root.closed()}
  }
  Component.onCompleted: {assignSlots();sync.restart();if(motion)enter.start()}
  ParallelAnimation {
    id:enter
    NumberAnimation{target:header;property:"opacity";from:0;to:1;duration:root.ms(380);easing.type:Easing.OutCubic}
    NumberAnimation{target:headerShift;property:"y";from:-Style.space(16);to:0;duration:root.ms(520);easing.type:Easing.OutBack}
  }

  Item {
    id:header;objectName:"t3MosaicHeader"
    anchors{left:parent.left;right:parent.right;top:parent.top}
    height:Style.space(58)
    transform:Translate{id:headerShift}
    RowLayout {
      anchors.fill:parent;anchors.leftMargin:Style.space(24);anchors.rightMargin:Style.space(14)
      spacing:Style.space(10)
      T3Icon {
        name:"layout-grid";implicitWidth:Style.space(16);tint:Color.accent
        // The grid icon turns a quarter each time the layout changes.
        rotation:root.count*90
        Behavior on rotation{NumberAnimation{duration:root.ms(620);easing.type:Easing.OutBack}}
      }
      Text {text:T3I18n.tr("Thread mosaic");color:root.fg;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
      // Live summary of everything that matches the filters; counts bump as they change.
      Row {
        spacing:Style.space(12);Layout.leftMargin:Style.space(6)
        Repeater {
          model:[{n:root.tally.live,label:"Working",tone:T3Palette.working,live:true},{n:root.tally.attention,label:"Waiting",tone:T3Palette.waiting,live:false},{n:root.tally.done,label:"Done",tone:T3Palette.done,live:false}].filter(function(c){return c.n>0})
          Row {
            id:tallyItem
            required property var modelData
            spacing:Style.space(5)
            Rectangle {
              anchors.verticalCenter:parent.verticalCenter;width:Style.space(6);height:width;radius:width/2;color:tallyItem.modelData.tone
              SequentialAnimation on scale {running:tallyItem.modelData.live&&root.motion&&root.skyMoving;loops:Animation.Infinite;NumberAnimation{to:1.6;duration:700;easing.type:Easing.InOutSine}NumberAnimation{to:1;duration:700;easing.type:Easing.InOutSine}}
            }
            Text {
              id:tallyText;text:tallyItem.modelData.n+" "+T3I18n.tr(tallyItem.modelData.label);color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText
              property int n:tallyItem.modelData.n
              onNChanged:if(root.motion)bump.restart()
              SequentialAnimation{id:bump;NumberAnimation{target:tallyText;property:"scale";to:1.25;duration:120;easing.type:Easing.OutCubic}NumberAnimation{target:tallyText;property:"scale";to:1;duration:380;easing.type:Easing.OutBack}}
            }
          }
        }
      }
      Item {Layout.fillWidth:true}
      // Views: 1–8, as a segmented control whose pill springs to the choice.
      Rectangle {
        objectName:"t3MosaicCount"
        Accessible.role:Accessible.PageTabList;Accessible.name:T3I18n.tr("Views")
        implicitWidth:viewRow.implicitWidth+Style.space(6);implicitHeight:Style.space(28)
        radius:Style.space(8);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.045);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08)
        Rectangle {
          y:viewRow.y;height:Style.space(22);width:Style.space(22);radius:Style.space(6)
          x:viewRow.x+(root.count-1)*(width+viewRow.spacing)
          color:Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.2)
          border.width:1;border.color:Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.35)
          Behavior on x{enabled:root.motion;SpringAnimation{spring:5;damping:0.32;epsilon:0.25}}
        }
        Row {
          id:viewRow;anchors.centerIn:parent;spacing:Style.space(2)
          Repeater {
            model:8
            Rectangle {
              id:viewButton
              required property int index
              readonly property bool current:root.count===index+1
              width:Style.space(22);height:Style.space(22);radius:Style.space(6)
              color:viewHover.hovered&&!current?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.07):"transparent"
              Behavior on color{ColorAnimation{duration:140}}
              Text {
                anchors.centerIn:parent;text:String(viewButton.index+1);color:viewButton.current?Color.accent:root.dim;font.pixelSize:Style.font.caption;font.weight:viewButton.current?Font.DemiBold:Font.Normal
                scale:viewButton.current?1.15:viewHover.hovered?1.08:1
                Behavior on scale{NumberAnimation{duration:root.ms(260);easing.type:Easing.OutBack}}
                Behavior on color{ColorAnimation{duration:root.ms(200)}}
              }
              HoverHandler{id:viewHover;cursorShape:Qt.PointingHandCursor}
              TapHandler{onTapped:root.bridge.rememberView({mosaicCount:viewButton.index+1})}
              Accessible.role:Accessible.PageTab;Accessible.name:String(index+1)
            }
          }
        }
      }
      T3IconButton {name:"list-filter";tint:root.dim;tooltipText:T3I18n.tr("Filter by state, project and host");onClicked:root.filtersRequested()}
      Row {
        visible:root.pages>1;spacing:Style.space(2)
        T3IconButton {name:"chevron-left";tint:root.dim;enabled:root.page>0;tooltipText:T3I18n.tr("Previous");onClicked:root.turn(-1)}
        Text {anchors.verticalCenter:parent.verticalCenter;text:(root.page+1)+" / "+root.pages;color:root.dim;font.pixelSize:Style.font.caption}
        T3IconButton {name:"chevron-right";tint:root.dim;enabled:root.page+1<root.pages;tooltipText:T3I18n.tr("Next");onClicked:root.turn(1)}
      }
      T3IconButton {objectName:"t3MosaicClose";name:"x";tint:root.dim;tooltipText:T3I18n.tr("Close");onClicked:root.dismiss()}
    }
  }
  Column {
    anchors.centerIn:board;spacing:Style.space(10)
    opacity:root.shown.length?0:1;visible:opacity>0
    scale:root.shown.length?0.9:1
    Behavior on opacity{NumberAnimation{duration:root.ms(320)}}
    Behavior on scale{NumberAnimation{duration:root.ms(420);easing.type:Easing.OutBack}}
    T3Icon {anchors.horizontalCenter:parent.horizontalCenter;name:"layout-grid";implicitWidth:Style.space(28);tint:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.3)}
    Text {anchors.horizontalCenter:parent.horizontalCenter;text:T3I18n.tr("No threads match these filters");color:root.dim;font.pixelSize:Style.font.body}
  }
  // The board places every slot itself so a new view, a column appearing as
  // the window widens, or a slot leaving all glide instead of jumping. Slots
  // past the chosen count wait below the board and rise in when needed.
  Item {
    id:board;objectName:"t3MosaicBoard"
    anchors{left:parent.left;right:parent.right;top:header.bottom;bottom:parent.bottom;leftMargin:Style.space(16);rightMargin:Style.space(16);bottomMargin:Style.space(16)}
    readonly property int wanted:root.count<=1?1:root.count<=4?2:root.count<=6?3:4
    readonly property int columns:Math.max(1,Math.min(wanted,Math.floor(width/Style.space(300))))
    readonly property int lines:Math.ceil(root.count/columns)
    readonly property real gap:Style.space(12)
    readonly property real cellHeight:Math.max(0,(height-(lines-1)*gap)/lines)
    // The last line stretches its tiles across the full width.
    function cell(i){
      var line=Math.floor(i/columns),col=i%columns
      if(i>=root.count){var w0=(width-(columns-1)*gap)/columns;return Qt.rect(col*(w0+gap),height+gap+Math.floor((i-root.count)/columns)*Style.space(40),w0,cellHeight)}
      var across=line===lines-1?root.count-line*columns:columns,w=(width-(across-1)*gap)/across
      return Qt.rect(col*(w+gap),line*(cellHeight+gap),w,cellHeight)
    }
    Repeater {
      model:8
      Item {
        id:tile
        required property int index
        readonly property bool present:index<root.count
        readonly property alias messageList:messages
        readonly property rect slot:board.cell(index)
        x:slot.x;y:slot.y;width:slot.width;height:slot.height
        Behavior on x{enabled:root.motion;NumberAnimation{duration:root.ms(560);easing.type:Easing.OutQuint}}
        Behavior on y{enabled:root.motion;NumberAnimation{duration:root.ms(620);easing.type:Easing.OutQuint}}
        Behavior on width{enabled:root.motion;NumberAnimation{duration:root.ms(560);easing.type:Easing.OutQuint}}
        Behavior on height{enabled:root.motion;NumberAnimation{duration:root.ms(620);easing.type:Easing.OutQuint}}
        opacity:present?1:0;visible:opacity>0.01
        scale:present?1:0.9
        Behavior on opacity{NumberAnimation{duration:root.ms(present?420:260);easing.type:Easing.OutCubic}}
        Behavior on scale{NumberAnimation{duration:root.ms(520);easing.type:Easing.OutBack}}
        // A leaving slot keeps its thread until it has faded.
        readonly property var current:present&&root.slotKeys[index]?(root.byKey[root.slotKeys[index]] || null):null
        property var thread:null
        onCurrentChanged:if(present)thread=current
        onPresentChanged:if(present)thread=current;else clearThread.restart()
        Timer {id:clearThread;interval:root.ms(320)+20;onTriggered:if(!tile.present)tile.thread=null}
        readonly property var snapshot:thread?(root.bridge.mosaicData[thread.key] || {}):({})
        readonly property var rows:(snapshot.messages || []).filter(function(r){return r.role==="user"||r.role==="assistant"||r.role==="activity"}).slice(-40)
        readonly property string phase:root.phaseOf(thread,snapshot)
        property color tone:root.toneOf(phase)
        Behavior on tone{ColorAnimation{duration:root.ms(520)}}
        readonly property bool live:["think","read","edit","execute","respond","agent","work"].indexOf(phase)>=0
        readonly property bool attention:phase==="attention"
        readonly property bool selected:!!thread&&thread.key===root.bridge.selectedKey
        readonly property string identity:thread?thread.key:""
        readonly property bool skyMoving:root.skyMoving
        readonly property int changeStyle:["star","flash"].indexOf(root.effects.change)
        function open(){if(!thread)return;root.bridge.selectThread(thread.environmentId,thread.id);root.dismiss()}

        // A thread arriving in this slot: tiles cascade in on open, flip toward
        // the page being turned, and rise when a filter or search swaps them.
        onIdentityChanged:{rowSync.restart();if(identity&&root.motion){arrive.delay=tile.index*(root.flipDir?45:55);arrive.dir=root.flipDir;arrive.restart()}}
        SequentialAnimation {
          id:arrive
          property int delay:0
          property int dir:0
          PropertyAction{target:body;property:"opacity";value:0}
          PropertyAction{target:flip;property:"angle";value:arrive.dir*-75}
          PropertyAction{target:rise;property:"y";value:arrive.dir?0:Style.space(26)}
          PropertyAction{target:body;property:"scale";value:arrive.dir?1:0.94}
          PauseAnimation{duration:arrive.delay}
          ParallelAnimation {
            NumberAnimation{target:body;property:"opacity";to:1;duration:root.ms(320);easing.type:Easing.OutCubic}
            NumberAnimation{target:flip;property:"angle";to:0;duration:root.ms(620);easing.type:Easing.OutBack;easing.overshoot:1.1}
            NumberAnimation{target:rise;property:"y";to:0;duration:root.ms(560);easing.type:Easing.OutQuint}
            NumberAnimation{target:body;property:"scale";to:1;duration:root.ms(560);easing.type:Easing.OutBack;easing.overshoot:1.3}
          }
        }
        // Finishing, failing or asking for input plays the row's state burst
        // and lifts the tile for a beat.
        onPhaseChanged:{
          if(skyMoving&&changeStyle>=0&&(phase==="done"||phase==="attention"||phase==="failed")){burstAnimation.stop();burstAnimation.from=0;burstAnimation.start()}
          if(root.motion&&thread){phasePop.restart();if(phase==="done"||phase==="attention"||phase==="failed")nudge.restart()}
        }
        SequentialAnimation{id:nudge;NumberAnimation{target:card;property:"scale";to:1.025;duration:root.ms(160);easing.type:Easing.OutCubic}NumberAnimation{target:card;property:"scale";to:1;duration:root.ms(520);easing.type:Easing.OutBack;easing.overshoot:2}}
        // A new message lights the border, like a notification in place.
        function ping(){if(!root.motion)return;pingAnimation.restart()}
        SequentialAnimation{id:pingAnimation;NumberAnimation{target:pingRing;property:"opacity";from:0;to:0.9;duration:root.ms(120)}NumberAnimation{target:pingRing;property:"opacity";to:0;duration:root.ms(900);easing.type:Easing.OutCubic}}

        // Rows are a keyed model so streaming updates edit in place and only
        // new rows animate in.
        ListModel {id:rowModel;dynamicRoles:true}
        property var prints:({})
        property var fresh:({})
        property string synced:""
        function rowId(r,i){return String(r.id || r.key || (r.role+":"+i))}
        function syncRows(){
          var rows=tile.rows,want={},bulk=tile.synced!==tile.identity || !rowModel.count,prints=bulk?{}:tile.prints,now=Date.now(),added=false,i,k
          if(bulk)rowModel.clear()
          var ids=rows.map(function(r,n){return tile.rowId(r,n)})
          ids.forEach(function(id){want[id]=true})
          for(k=rowModel.count-1;k>=0;k--)if(!want[rowModel.get(k).rowId]){delete prints[rowModel.get(k).rowId];rowModel.remove(k)}
          for(i=0;i<rows.length;i++){
            var id=ids[i],print=JSON.stringify(rows[i])
            if(i<rowModel.count&&rowModel.get(i).rowId===id){if(prints[id]!==print)rowModel.set(i,{row:rows[i],rowId:id})}
            else{
              var at=-1;for(k=i+1;k<rowModel.count;k++)if(rowModel.get(k).rowId===id){at=k;break}
              if(at>=0){rowModel.move(at,i,1);rowModel.set(i,{row:rows[i],rowId:id})}
              else{if(!bulk){tile.fresh[id]=now;added=true}rowModel.insert(i,{row:rows[i],rowId:id})}
            }
            prints[id]=print
          }
          if(bulk)messages.follow=true
          tile.prints=prints;tile.synced=tile.identity
          if(added)tile.ping()
        }
        // A timer owned by the tile, so a sync never outlives it.
        Timer{id:rowSync;interval:0;onTriggered:tile.syncRows()}
        onRowsChanged:rowSync.restart()

        HoverHandler{id:tileHover}
        Item {
          id:body;anchors.fill:parent
          transform:[Translate{id:rise},Rotation{id:flip;origin.x:body.width/2;origin.y:body.height/2;axis{x:0;y:1;z:0}}]
          // A slow glow around tiles whose agent is busy, in the phase colour.
          Rectangle {
            id:glowRing
            anchors.fill:card;anchors.margins:-Style.space(3);radius:card.radius+Style.space(3)
            color:"transparent";border.width:Style.space(3)
            border.color:Qt.rgba(tile.tone.r,tile.tone.g,tile.tone.b,1)
            visible:!!tile.thread&&(tile.live||tile.attention)&&root.motion
            opacity:glowTick.running?0.07+0.11*(0.5+0.5*Math.sin(glowTick.time*(tile.attention?2.6:1.8)+tile.index)):0.1
            T3Tick{id:glowTick;running:glowRing.visible&&root.skyMoving;pace:"ambient"}
          }
          Rectangle {
            id:pingRing
            anchors.fill:card;anchors.margins:-Style.space(1);radius:card.radius+Style.space(1)
            color:"transparent";border.width:Style.space(2);border.color:tile.tone;opacity:0
          }
          Rectangle {
            id:card
            anchors.fill:parent;visible:!!tile.thread
            radius:Style.space(14)
            color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,tileHover.hovered?0.065:0.045)
            border.width:1
            border.color:tile.selected?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.7):tile.live||tile.attention?Qt.rgba(tile.tone.r,tile.tone.g,tile.tone.b,0.38):Qt.rgba(root.fg.r,root.fg.g,root.fg.b,tileHover.hovered?0.16:0.08)
            Behavior on color{ColorAnimation{duration:160}}
            Behavior on border.color{ColorAnimation{duration:root.ms(420)}}
            clip:true
            // The phase colour washes the top of the card and drifts while it works.
            Rectangle {
              anchors{left:parent.left;right:parent.right;top:parent.top}
              height:Math.min(parent.height,Style.space(110));radius:parent.radius
              opacity:tile.live||tile.attention?1:tile.phase==="failed"?0.8:0
              Behavior on opacity{NumberAnimation{duration:root.ms(600)}}
              gradient:Gradient{
                GradientStop{position:0;color:Qt.rgba(tile.tone.r,tile.tone.g,tile.tone.b,0.15)}
                GradientStop{position:1;color:Qt.rgba(tile.tone.r,tile.tone.g,tile.tone.b,0)}
              }
            }
            ShaderEffect {
              id:aura;objectName:"t3MosaicAura_"+tile.index
              anchors.fill:parent;z:3
              property real burst:0
              readonly property bool bursting:burst>0&&burst<1
              readonly property int workingStyleIndex:["comet","pulse"].indexOf(root.effects.working)
              readonly property int inputStyleIndex:!tile.skyMoving&&root.effects.input!=="off"?2:["glints","pulse","static"].indexOf(root.effects.input)
              property real working:tile.live&&tile.skyMoving&&workingStyleIndex>=0?1:0
              property real attention:tile.attention&&inputStyleIndex>=0?1:0
              property real intensity:root.effects.intensity || 1
              property real workingStyle:Math.max(0,workingStyleIndex)
              property real attentionStyle:Math.max(0,inputStyleIndex)
              property real burstStyle:Math.max(0,tile.changeStyle)
              Behavior on working{NumberAnimation{duration:600}}
              Behavior on attention{NumberAnimation{duration:600}}
              visible:working>0||attention>0||bursting
              property real clock:visible&&root.sky?root.sky.clock:1.2
              property real radius:card.radius
              property real seed:{var h=0;for(var i=0;i<tile.identity.length;i++)h=(h*31+tile.identity.charCodeAt(i))%9973;return h/9973}
              property vector2d size:Qt.vector2d(width,height)
              property color tone:tile.tone
              property color alert:T3Palette.waiting
              property color flash:tile.tone
              blending:true
              fragmentShader:Qt.resolvedUrl("../assets/shaders/threadaura.frag.qsb")
              NumberAnimation{id:burstAnimation;target:aura;property:"burst";to:1;duration:1300}
              T3Tick{running:aura.visible&&(aura.working>0||aura.attention>0)&&tile.skyMoving;pace:"smooth"}
            }
            TapHandler{onDoubleTapped:tile.open()}
            ColumnLayout {
              anchors.fill:parent;anchors.margins:Style.space(14);anchors.bottomMargin:Style.space(10);spacing:Style.space(7)
              RowLayout {
                Layout.fillWidth:true;spacing:Style.space(6)
                T3ProjectIcon {
                  visible:!!tile.thread&&!!tile.thread.projectKey
                  project:tile.thread?((root.bridge.state.projects || []).find(function(p){return p.key===tile.thread.projectKey}) || ({})):({});projects:root.bridge.state.projects || []
                  monogram:tile.thread?(tile.thread.projectMonogram || "PR"):"PR";projectKey:tile.thread?(tile.thread.projectKey || ""):""
                  Layout.preferredWidth:Style.space(16);Layout.preferredHeight:Style.space(16);radius:Style.space(4)
                }
                Text {visible:!!tile.thread&&!!tile.thread.project;text:tile.thread?tile.thread.project || "":"";color:root.dim;elide:Text.ElideRight;Layout.maximumWidth:tile.width*0.3;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
                Text {visible:!!tile.thread&&!!tile.thread.project;text:"/";color:root.dim;font.pixelSize:Style.font.body}
                Text {Layout.fillWidth:true;text:tile.thread?tile.thread.title:"";color:root.fg;elide:Text.ElideRight;textFormat:Text.PlainText;font.pixelSize:Style.font.body;font.weight:Font.Medium}
                T3IconButton {
                  name:"maximize";implicitWidth:Style.space(24);implicitHeight:implicitWidth;tint:root.dim;tooltipText:T3I18n.tr("Open thread");onClicked:tile.open()
                  opacity:tileHover.hovered||activeFocus?1:0.45;scale:tileHover.hovered?1.08:1
                  Behavior on opacity{NumberAnimation{duration:root.ms(180)}}
                  Behavior on scale{NumberAnimation{duration:root.ms(260);easing.type:Easing.OutBack}}
                }
              }
              // The phase line: its glyph pops and its words slide in whenever the agent moves on.
              RowLayout {
                id:phaseRow
                Layout.fillWidth:true;spacing:Style.space(7)
                PhaseGlyph {id:phaseGlyph;phase:tile.phase;tint:tile.tone;live:tile.live||tile.attention;implicitWidth:Style.space(15)}
                T3ActivityText {
                  id:phaseText;objectName:"t3MosaicPhase_"+tile.index
                  Layout.fillWidth:true;active:tile.live;tint:tile.tone
                  text:root.labelOf(tile.phase,tile.thread,tile.snapshot)
                  transform:Translate{id:phaseSlide}
                }
                T3Icon {name:"server";implicitWidth:Style.space(12);tint:root.dim;visible:!!tile.thread}
                Text {text:tile.thread?root.environmentName(tile.thread.environmentId):"";color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;Layout.maximumWidth:tile.width*0.3;textFormat:Text.PlainText}
              }
              ParallelAnimation {
                id:phasePop
                NumberAnimation{target:phaseGlyph;property:"scale";from:0.3;to:1;duration:root.ms(560);easing.type:Easing.OutBack;easing.overshoot:2.4}
                NumberAnimation{target:phaseText;property:"opacity";from:0;to:1;duration:root.ms(360);easing.type:Easing.OutCubic}
                NumberAnimation{target:phaseSlide;property:"x";from:-Style.space(10);to:0;duration:root.ms(420);easing.type:Easing.OutCubic}
              }
              PhaseStrip {Layout.fillWidth:true;phase:tile.phase;tint:tile.tone;live:tile.live||tile.attention}
              Text {
                Layout.fillWidth:true;visible:!!tile.thread&&(!tile.snapshot.connected||!!tile.snapshot.error)&&!tile.rows.length
                text:tile.snapshot.error?T3I18n.error(tile.snapshot.error):T3I18n.tr(tile.snapshot.loaded?"Reconnecting…":"Loading…")
                color:root.dim;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption
                SequentialAnimation on opacity{running:parent.visible&&root.motion&&!tile.snapshot.error;loops:Animation.Infinite;NumberAnimation{to:0.4;duration:700;easing.type:Easing.InOutSine}NumberAnimation{to:1;duration:700;easing.type:Easing.InOutSine}}
              }
              Item {
                Layout.fillWidth:true;Layout.fillHeight:true
                ListView {
                  id:messages;objectName:"t3MosaicMessages_"+tile.index
                  anchors.fill:parent;clip:true;spacing:Style.space(10)
                  // Follows the newest text like the chat; only the user scrolling
                  // up lets go, and reaching the end again resumes following.
                  property bool follow:true
                  readonly property bool nearEnd:originY+contentHeight-height-contentY<Style.space(24)
                  readonly property bool userScrolling:dragging||flicking||messageBar.pressed===true
                  onContentYChanged:if(userScrolling)follow=nearEnd
                  onMovementEnded:follow=nearEnd
                  onContentHeightChanged:if(follow)tail.restart()
                  onCountChanged:if(follow)tail.restart()
                  onHeightChanged:if(follow)tail.restart()
                  Timer{id:tail;interval:0;onTriggered:{messages.positionViewAtEnd();settle.restart()}}
                  // Rich text and new rows settle their height a frame later.
                  Timer{id:settle;interval:60;onTriggered:if(messages.follow&&!messages.nearEnd)messages.positionViewAtEnd()}
                  model:rowModel
                  displaced:Transition{NumberAnimation{properties:"y";duration:root.ms(280);easing.type:Easing.OutCubic}}
                  Controls.ScrollBar.vertical:Controls.ScrollBar{id:messageBar;policy:Controls.ScrollBar.AsNeeded}
                  T3Wheel {id:messageWheel;flickable:messages;onUserScrolled:{messages.follow=false;wheelSettle.restart()}}
                  // Once the wheel stops, following resumes only at the end.
                  Timer{id:wheelSettle;interval:180;onTriggered:if(messageWheel.running)restart();else messages.follow=messages.nearEnd}
                  delegate:Item {
                    id:entry
                    required property var row
                    required property string rowId
                    required property int index
                    readonly property bool user:row.role==="user"
                    readonly property bool activity:row.role==="activity"
                    readonly property bool streaming:row.role==="assistant"&&row.streaming===true
                    width:messages.width
                    height:user?userBubble.height:activity?activityLine.height:answer.implicitHeight
                    // Only rows that just arrived animate; scrolling back never replays them.
                    transform:Translate{id:entryShift}
                    Component.onCompleted:{var born=tile.fresh[rowId];if(born&&Date.now()-born<1500){delete tile.fresh[rowId];entryIn.start()}}
                    ParallelAnimation {
                      id:entryIn
                      NumberAnimation{target:entry;property:"opacity";from:0;to:1;duration:root.ms(320);easing.type:Easing.OutCubic}
                      NumberAnimation{target:entryShift;property:"y";from:Style.space(16);to:0;duration:root.ms(460);easing.type:Easing.OutBack}
                      NumberAnimation{target:entryShift;property:"x";from:entry.user?Style.space(24):0;to:0;duration:root.ms(420);easing.type:Easing.OutCubic}
                    }
                    // The user's prompt: the chat's bubble, right-aligned.
                    Rectangle {
                      id:userBubble;visible:entry.user
                      anchors.right:parent.right
                      width:Math.min(parent.width*0.86,userText.implicitWidth+Style.space(22))
                      height:visible?userText.implicitHeight+Style.space(16):0
                      radius:Style.space(14);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.07)
                      Text {
                        id:userText;x:Style.space(11);y:Style.space(8)
                        width:Math.min(entry.width*0.86-Style.space(22),implicitWidth)
                        text:entry.user?String(entry.row.text || ""):"";textFormat:Text.PlainText;wrapMode:Text.Wrap
                        maximumLineCount:6;elide:Text.ElideRight;color:root.fg;font.pixelSize:Style.font.body
                      }
                    }
                    // Tool work: one quiet line, shimmering while it runs.
                    RowLayout {
                      id:activityLine;visible:entry.activity;width:parent.width;height:visible?Style.space(20):0;spacing:Style.space(6)
                      readonly property var act:entry.activity?(entry.row.activity || {}):({})
                      readonly property string phase:{var c=act.category || ({reasoning:"think",command_execution:"execute",file_change:"edit",file_search:"search",web_search:"web",subagent:"agent"})[act.type] || "tool";return ({think:"think",read:"read",search:"read",web:"read",edit:"edit",execute:"execute",agent:"agent"})[c] || "work"}
                      PhaseGlyph {phase:activityLine.phase;live:!!activityLine.act.active;tint:activityLine.act.failed&&!activityLine.act.active?Color.urgent:Qt.rgba(root.toneOf(activityLine.phase).r,root.toneOf(activityLine.phase).g,root.toneOf(activityLine.phase).b,activityLine.act.active?1:0.6);implicitWidth:Style.space(12)}
                      T3ActivityText {Layout.fillWidth:true;active:!!activityLine.act.active&&entry.activity;tint:activityLine.act.failed&&!activityLine.act.active?Color.urgent:root.fg;text:entry.activity?root.bridge.activityText(activityLine.act):""}
                    }
                    // The answer, rendered like the conversation; a caret types at its end.
                    TextEdit {
                      id:answer;visible:entry.row.role==="assistant"
                      width:parent.width;readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap
                      textFormat:entry.row.rendered&&root.renderer?TextEdit.RichText:TextEdit.PlainText
                      text:!visible?"":entry.row.rendered&&root.renderer?root.renderer(entry.row,width):String(entry.row.text || "")
                      color:root.fg;selectionColor:Color.accent;selectedTextColor:Color.popups.background;font.pixelSize:Style.font.body
                      onLinkActivated:function(link){root.bridge.openReference(link)}
                      HoverHandler{cursorShape:answer.hoveredLink?Qt.PointingHandCursor:Qt.IBeamCursor}
                      Rectangle {
                        visible:entry.streaming
                        readonly property rect at:answer.positionToRectangle(answer.length)
                        x:at.x+Style.space(2);y:at.y+at.height*0.12;width:Math.max(2,Style.spaceReal(2));height:at.height*0.8;radius:width/2
                        color:tile.tone
                        SequentialAnimation on opacity{running:entry.streaming;loops:Animation.Infinite;NumberAnimation{to:0.15;duration:420;easing.type:Easing.InOutSine}NumberAnimation{to:1;duration:420;easing.type:Easing.InOutSine}}
                      }
                    }
                  }
                }
              }
              // Waiting for the user: the banner slides up and glows until answered.
              Item {
                Layout.fillWidth:true
                Layout.preferredHeight:tile.attention?Style.space(34):0
                Behavior on Layout.preferredHeight{NumberAnimation{duration:root.ms(420);easing.type:Easing.OutBack}}
                visible:Layout.preferredHeight>0.5;clip:true
                Rectangle {
                  objectName:"t3MosaicAttention_"+tile.index
                  anchors{left:parent.left;right:parent.right;bottom:parent.bottom}
                  height:Style.space(34);radius:Style.space(10)
                  opacity:tile.attention?1:0
                  Behavior on opacity{NumberAnimation{duration:root.ms(300)}}
                  color:Qt.rgba(T3Palette.waiting.r,T3Palette.waiting.g,T3Palette.waiting.b,0.1)
                  border.width:1;border.color:Qt.rgba(T3Palette.waiting.r,T3Palette.waiting.g,T3Palette.waiting.b,0.35)
                  RowLayout {
                    anchors.fill:parent;anchors.leftMargin:Style.space(10);anchors.rightMargin:Style.space(4);spacing:Style.space(8)
                    T3Icon {
                      name:"message-circle-question";implicitWidth:Style.space(14);tint:T3Palette.waiting
                      SequentialAnimation on rotation{running:tile.attention&&root.motion&&root.skyMoving;loops:Animation.Infinite;NumberAnimation{to:-12;duration:120}NumberAnimation{to:12;duration:160}NumberAnimation{to:0;duration:120}PauseAnimation{duration:2200}}
                    }
                    Text {Layout.fillWidth:true;text:root.labelOf("attention",tile.thread,tile.snapshot);color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
                    T3PickerButton {primary:false;implicitHeight:Style.space(26);leftPadding:Style.space(10);rightPadding:Style.space(10);text:T3I18n.tr("Open thread");onClicked:tile.open()}
                  }
                }
              }
            }
          }
          // An empty slot keeps the grid's rhythm without shouting.
          Rectangle {
            anchors.fill:parent;visible:!tile.thread
            radius:Style.space(14);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.018)
            border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05)
            Column {
              anchors.centerIn:parent;spacing:Style.space(8)
              T3Icon {anchors.horizontalCenter:parent.horizontalCenter;name:"layout-grid";implicitWidth:Style.space(18);tint:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.22)}
              Text {anchors.horizontalCenter:parent.horizontalCenter;text:T3I18n.tr("No thread");color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.35);font.pixelSize:Style.font.caption}
            }
          }
        }
      }
    }
  }
}
