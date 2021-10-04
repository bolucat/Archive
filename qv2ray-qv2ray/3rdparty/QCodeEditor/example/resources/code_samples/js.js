// example of a JS code to test JS syntax highlighter
// source: 
//    https://stackoverflow.com/questions/61301013/why-adapter-function-changing-my-num-array
//    https://stackoverflow.com/questions/61301471/how-can-i-filter-undefined-from-an-object

var num = []

// creates a copy of a passed list, pushes 1 to it and returns it
function push(nums) {
  var orig = nums.map(x => x)
  var newNum = pushed()
  num = orig
  return newNum
}

/*
This is a helper function
*/
function pushed() {
  num.push(1)
  return num
}

function filterFunnel() {
  let filterLists = $("#funnel-filters").children().filter(".filter");
  let query = `SELECT * FROM funnel WHERE `;
  let count = 0;
  for(i = 0; i < Object.keys(filterLists).length; i++) {
      let dom = filterLists[i];
      if(dom === undefined){
          return query;
      } else {
          if(dom.value === "Select" || dom.value === "") {

          } else {
              if(count >= 1) {
                  query += ` AND ${dom.id} = ${dom.value}`;
              }else{
                  query += `${dom.id} = ${dom.value} `
              }
              count += 1;
          }   
      }
  }
}

console.log("new", push(num))
console.log("old", num)