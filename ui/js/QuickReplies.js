.pragma library
// Local language hints: never send drafts to a translation service.
var defaults = {
 nl:["Ga door.","Ga verder.","Stop.","Controleer de wijzigingen.","Voer de tests uit.","Leg het resultaat uit."],
 pl:["Kontynuuj.","Dalej.","Zatrzymaj się.","Sprawdź zmiany.","Uruchom testy.","Wyjaśnij wynik."],
 ru:["Продолжай.","Дальше.","Остановись.","Проверь изменения.","Запусти тесты.","Объясни результат."],
 uk:["Продовжуй.","Далі.","Зупинись.","Перевір зміни.","Запусти тести.","Поясни результат."],
 ja:["続けて。","先に進めて。","止めて。","変更を検証して。","テストを実行して。","結果を説明して。"],
 ko:["계속해.","진행해.","멈춰.","변경 사항을 검증해.","테스트를 실행해.","결과를 설명해."],
 zh:["继续。","接着做。","停止。","验证更改。","运行测试。","解释结果。"],
 ar:["تابع.","استمر.","توقف.","تحقق من التغييرات.","شغّل الاختبارات.","اشرح النتيجة."],
 tr:["Devam et.","İlerle.","Dur.","Değişiklikleri doğrula.","Testleri çalıştır.","Sonucu açıkla."],
 sv:["Fortsätt.","Gå vidare.","Stanna.","Kontrollera ändringarna.","Kör testerna.","Förklara resultatet."],
 da:["Fortsæt.","Gå videre.","Stop.","Kontrollér ændringerne.","Kør testene.","Forklar resultatet."],
 no:["Fortsett.","Gå videre.","Stopp.","Kontroller endringene.","Kjør testene.","Forklar resultatet."],
 fi:["Jatka.","Jatka eteenpäin.","Pysähdy.","Tarkista muutokset.","Suorita testit.","Selitä tulos."],
 cs:["Pokračuj.","Dále.","Zastav.","Ověř změny.","Spusť testy.","Vysvětli výsledek."],
 en:["Continue.","Go on.","Stop.","Validate the changes.","Run the tests.","Explain the result."],
 es:["Continúa.","Sigue.","Detente.","Valida los cambios.","Ejecuta las pruebas.","Explica el resultado."],
 pt:["Continue.","Prossiga.","Pare.","Valide as alterações.","Execute os testes.","Explique o resultado."],
 fr:["Continue.","Poursuis.","Arrête.","Valide les modifications.","Lance les tests.","Explique le résultat."],
 de:["Fahre fort.","Mach weiter.","Stopp.","Prüfe die Änderungen.","Führe die Tests aus.","Erkläre das Ergebnis."],
 it:["Continua.","Vai avanti.","Fermati.","Verifica le modifiche.","Esegui i test.","Spiega il risultato."]
}
var hints = {
 nl:/\b(het|wijzigingen|uitvoeren|resultaat|verder|alsjeblieft)\b/gi,
 pl:/\b(proszę|kontynuuj|zmiany|sprawdź|uruchom|wynik)\b/gi,
 tr:/\b(devam|değişiklikleri|doğrula|çalıştır|lütfen|sonucu)\b/gi,
 sv:/\b(fortsätt|ändringarna|kör|förklara|resultatet)\b/gi,
 da:/\b(fortsæt|ændringerne|kør|forklar)\b/gi,
 no:/\b(fortsett|endringene|kjør|vennligst)\b/gi,
 fi:/\b(jatka|muutokset|suorita|selitä|tulos|haluan)\b/gi,
 cs:/\b(pokračuj|změny|spusť|vysvětli|výsledek)\b/gi,
 es:/\b(el|los|las|quiero|puedes|cambios|pruebas|implementa|funciones|sigue|contin[uú]a|valida|detente|por|para|una|usuario)\b/gi,
 en:/\b(the|please|want|can|changes|tests|implement|functions|continue|stop|validate|with|this|that|user)\b/gi,
 pt:/\b(os|uma|voc[eê]|altera[çc][õo]es|testes|prossiga|pare|obrigado|quero|usu[aá]rio)\b/gi,
 fr:/\b(les|une|vous|avec|pour|veux|peux|arr[êe]te|modifications|r[ée]sultat|utilisateur)\b/gi,
 de:/\b(der|die|das|bitte|und|[äa]nderungen|pr[üu]fe|weiter|ergebnis|benutzer)\b/gi,
 it:/\b(gli|voglio|puoi|modifiche|verifica|esegui|risultato|fermati|utente)\b/gi
}
function detect(text) {
 text=String(text || "")
 if(/[ぁ-ヿ]/.test(text))return "ja"
 if(/[가-힣]/.test(text))return "ko"
 if(/[\u0600-\u06ff]/.test(text))return "ar"
 if(/[ієїґІЄЇҐ]/.test(text))return "uk"
 if(/[а-яА-ЯёЁ]/.test(text))return "ru"
 if(/[\u3400-\u9fff]/.test(text))return "zh"
 var winner="",score=0
 Object.keys(hints).forEach(function(l){var n=(String(text || "").match(hints[l]) || []).length;if(n>score){winner=l;score=n}})
 return winner
}
function language(draft,messages,fallback,choice) {
 if(choice&&choice!=="auto")return choice
 var detected=detect(draft)
 if(detected)return detected
 for(var i=messages.length-1;i>=0;i--)if(messages[i].role==="user"){detected=detect(messages[i].text);if(detected)return detected}
 return defaults[fallback]?fallback:"en"
}
function replies(language,custom) {return custom&&Array.isArray(custom[language])?custom[language]:(defaults[language] || defaults.en)}
function parse(text) {return String(text).split(/\r?\n/).map(function(s){return s.trim().slice(0,250)}).filter(function(s){return !!s}).slice(0,32)}
